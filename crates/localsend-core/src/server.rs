use crate::{
    utils, AppState, ClientMessage, ReceiveSession, ReceiveState, ReceiveStatus, Receiver,
    SendInfo, Sender, ServerMessage,
};
use axum::{
    body::Bytes,
    extract::{BodyStream, Query, State},
    http::StatusCode,
    routing::post,
    BoxError, Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use futures::{Stream, TryStreamExt};
use std::{collections::HashMap, path::Path};
use std::{io, net::SocketAddr};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;
use tracing::{info, trace};
use uuid::Uuid;

pub struct Server {
    certificate: rcgen::Certificate,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    pub fn new() -> Self {
        Self {
            certificate: utils::generate_tls_cert(),
        }
    }

    pub async fn start_server(&self, server_tx: Sender, client_rx: Receiver) {
        let cert_pem = self.certificate.serialize_pem().unwrap();
        let private_key_pem = self.certificate.serialize_private_key_pem();
        let config = RustlsConfig::from_pem(cert_pem.into_bytes(), private_key_pem.into_bytes())
            .await
            .unwrap();

        let app_state = Arc::new(Mutex::new(AppState {
            server_tx,
            client_rx,
            receive_session: None,
        }));

        let app = Router::new()
            .route("/api/localsend/v1/send-request", post(Self::send_request))
            .route("/api/localsend/v1/send", post(Self::incoming_send_post))
            .with_state(app_state);

        let addr = SocketAddr::from(([0, 0, 0, 0], crate::MULTICAST_PORT));
        info!("listening on {}", addr);
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }

    async fn send_request(
        State(session_state): State<ReceiveState>,
        Json(send_request): Json<crate::SendRequest>,
    ) -> Result<Json<HashMap<String, String>>, (StatusCode, String)> {
        trace!("got request {:#?}", send_request);

        let mut session = session_state.lock().await;
        if session.receive_session.is_some() {
            // reject incoming request if another session is ongoing
            return Err((StatusCode::CONFLICT, "Blocked by another sesssion".into()));
        }

        let _ = session.server_tx.send(ServerMessage::SendRequest);
        if let Some(ClientMessage::Decline) = session.client_rx.recv().await {
            return Err((StatusCode::FORBIDDEN, "User declined the request".into()));
        }

        // TODO: create destination_directory if it doesn't exist
        let state = session.receive_session.insert(ReceiveSession::new(
            send_request.device_info,
            "./test_files/".into(),
        ));

        let mut wanted_files: HashMap<String, String> = HashMap::new();
        send_request
            .files
            .into_iter()
            .for_each(|(file_id, file_info)| {
                let token = Uuid::new_v4();
                wanted_files.insert(file_id.clone(), token.to_string());
                state.files.insert(file_id.clone(), file_info);
                state.file_status.insert(file_id, ReceiveStatus::Waiting);
            });
        trace!("{:#?}", &wanted_files);
        trace!("{:#?}, ", &state.files);
        Ok(Json(wanted_files))
    }

    async fn incoming_send_post(
        State(session_state): State<ReceiveState>,
        params: Query<SendInfo>,
        file_stream: BodyStream,
    ) -> Result<(), (StatusCode, String)> {
        // NOTE: i shouldn't be locking session_state for the whole function but since we are only
        // receiving files one by one, it should be fine. Shouldn't be locking for the whole
        // function if we are going to receive multiple files at the same time.
        let mut session = session_state.lock().await;
        if session.receive_session.is_none() {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Call to /send without requesting a send".into(),
            ));
        }
        let _ = session.server_tx.send(ServerMessage::SendFileRequest);

        let mut receive_session = session.receive_session.as_mut().unwrap();
        receive_session.status = ReceiveStatus::Receiving;

        let file_id = params.file_id.clone();
        let path = Path::new(&receive_session.destination_directory)
            .join(&receive_session.files[&params.file_id].file_name);
        let result = stream_to_file(path, file_stream).await;

        receive_session
            .file_status
            .entry(file_id)
            .and_modify(|file_status| {
                *file_status = if result.is_ok() {
                    ReceiveStatus::Finished
                } else {
                    ReceiveStatus::FinishedWithErrors
                }
            });

        let all_finished = receive_session.files.iter().all(|(file_status_id, _)| {
            receive_session.file_status[file_status_id] == ReceiveStatus::Finished
                || receive_session.file_status[file_status_id] == ReceiveStatus::FinishedWithErrors
        });
        if all_finished {
            // TODO: add support for FinishedWithErrors and send message to bin crate before
            // setting receive_session to None
            receive_session.status = ReceiveStatus::Finished;
            session.receive_session = None;
        }
        return Ok(());
    }
}

// from: https://github.com/tokio-rs/axum/blob/main/examples/stream-to-file/src/main.rs
async fn stream_to_file<S, E>(path: PathBuf, stream: S) -> std::io::Result<()>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>, // BoxError is just - Box<dyn std::error::Error + Send + Sync>
{
    // Convert the stream into an `AsyncRead`.
    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
    let body_reader = StreamReader::new(body_with_io_error);
    futures::pin_mut!(body_reader);

    // Create the file. `File` implements `AsyncWrite`.
    let mut file = BufWriter::new(File::create(path).await?);

    // Copy the body into the file.
    tokio::io::copy(&mut body_reader, &mut file).await?;
    Ok(())
}
