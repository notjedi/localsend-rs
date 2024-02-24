use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
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
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
    sync::Mutex,
};
use tokio_util::io::StreamReader;
use tracing::{info, trace};
use uuid::Uuid;

use crate::{
    utils, AppState, ClientMessage, ReceiveSession, ReceiveState, ReceiveStatus, Receiver,
    SendInfo, SendRequest, Sender, ServerMessage,
};

pub struct Server {
    certificate: rcgen::Certificate,
    interface_addr: Ipv4Addr,
    multicast_port: u16,
}

impl Server {
    pub fn new(interface_addr: Ipv4Addr, multicast_port: u16) -> Self {
        Self {
            certificate: utils::generate_tls_cert(),
            interface_addr,
            multicast_port,
        }
    }

    pub async fn start_server(
        &self,
        server_tx: Sender<ServerMessage>,
        client_rx: Receiver<ClientMessage>,
    ) {
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
            .route(
                "/api/localsend/v1/send-request",
                post(Self::handle_send_request),
            )
            .route(
                "/api/localsend/v1/send",
                post(Self::handle_send_file_request),
            )
            .route(
                "/api/localsend/v1/cancel",
                post(Self::handle_cancel_request),
            )
            .with_state(app_state);

        let addr = SocketAddr::from((self.interface_addr, self.multicast_port));
        info!("listening on {}", addr);
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }

    async fn handle_cancel_request(
        State(session_state): State<ReceiveState>,
    ) -> Result<(), (StatusCode, String)> {
        let mut session = session_state.lock().await;
        if session.receive_session.is_none() {
            // reject incoming request if another session is ongoing
            return Err((
                StatusCode::BAD_REQUEST,
                "Cannot cancel a non existant session".into(),
            ));
        }

        // TODO(notjedi): check if cancel request is valid by comparing the ip address
        // TODO(notjedi): set session_state.receive_session to None
        // TODO(notjedi): clear buffer of sender_tx
        let _ = session.server_tx.send(ServerMessage::CancelSession);

        session.receive_session = None;
        Ok(())
    }

    async fn handle_send_request(
        State(session_state): State<ReceiveState>,
        Json(send_request): Json<SendRequest>,
    ) -> Result<Json<HashMap<String, String>>, (StatusCode, String)> {
        trace!("got request {:#?}", send_request);

        let mut session = session_state.lock().await;
        if session.receive_session.is_some() {
            // reject incoming request if another session is ongoing
            return Err((StatusCode::CONFLICT, "Blocked by another sesssion".into()));
        }

        let _ = session
            .server_tx
            .send(ServerMessage::SendRequest(send_request.clone()));
        let response = session.client_rx.recv().await;

        match response {
            Some(ClientMessage::Decline) | None => {
                Err((StatusCode::FORBIDDEN, "User declined the request".into()))
            }
            Some(ClientMessage::Allow(file_ids)) => {
                // TODO: create destination_directory if it doesn't exist
                let state = session.receive_session.insert(ReceiveSession::new(
                    send_request.device_info,
                    "./test_files/".into(),
                ));

                // TODO(notjedi): yo, why so many clones?
                let mut wanted_files: HashMap<String, String> = HashMap::new();
                file_ids.into_iter().for_each(|file_id| {
                    let token = Uuid::new_v4();
                    wanted_files.insert(file_id.clone(), token.to_string());
                    state
                        .files
                        .insert(file_id.clone(), send_request.files[&file_id].clone());
                    state.file_status.insert(file_id, ReceiveStatus::Waiting);
                });
                trace!("{:#?}", &wanted_files);
                trace!("{:#?}, ", &state.files);

                Ok(Json(wanted_files))
            }
        }
    }

    async fn handle_send_file_request(
        State(session_state): State<ReceiveState>,
        params: Query<SendInfo>,
        file_stream: BodyStream,
    ) -> Result<(), (StatusCode, String)> {
        // NOTE: i shouldn't be locking session_state for the whole function but since we are only
        // receiving files one by one, it should be fine. Shouldn't be locking for the whole
        // function if we are going to receive multiple files at the same time.

        let (file_id, path, sender) = {
            let mut session = session_state.lock().await;
            if session.receive_session.is_none() {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Call to /send without requesting a send".into(),
                ));
            }

            let _ = session
                .server_tx
                .send(ServerMessage::SendFileRequest((params.file_id.clone(), 0)));

            if !session
                .receive_session
                .as_ref()
                .unwrap()
                .files
                .contains_key(&params.file_id)
            {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Call to /send with unknown file id {}", params.file_id),
                ));
            }

            let mut receive_session = session.receive_session.as_mut().unwrap();
            receive_session.status = ReceiveStatus::Receiving;

            let file_id = params.file_id.clone();
            let path = Path::new(&receive_session.destination_directory)
                .join(&receive_session.files[&params.file_id].file_name);
            (file_id, path, session.server_tx.clone())
        };

        let result = stream_to_file(path, file_stream, file_id.clone(), sender).await;

        let mut session = session_state.lock().await;
        if session.receive_session.is_none() {
            // TODO(notjedi): should i return Ok(()) here?
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Session might have been cancelled while receiving file".into(),
            ));
        }
        let mut receive_session = session.receive_session.as_mut().unwrap();

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
        // TODO(notjedi): do i need to loop over everything and set the status?
        if all_finished {
            // TODO: add support for FinishedWithErrors and send message to bin crate before
            // setting receive_session to None
            receive_session.status = ReceiveStatus::Finished;
            session.receive_session = None;
        }
        Ok(())
    }
}

// taken and modified from: https://github.com/tokio-rs/axum/blob/main/examples/stream-to-file/src/main.rs
async fn stream_to_file<S, E>(
    path: PathBuf,
    stream: S,
    file_id: String,
    sender: Sender<ServerMessage>,
) -> std::io::Result<()>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>, // BoxError is just - Box<dyn std::error::Error + Send + Sync>
{
    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
    let body_reader = StreamReader::new(body_with_io_error);
    futures::pin_mut!(body_reader);

    let file = File::create(path).await?;
    let mut file_buf = BufWriter::with_capacity(16384, file);

    // read 1024 * 16 bytes on each read call
    // can i directly write to the file buffer? rn we are copying data to a buf and writing that to the file
    let mut buf = [0u8; 16384];
    loop {
        match body_reader.read(&mut buf[..]).await {
            Ok(0) => {
                break;
            }
            Ok(len) => {
                // TODO: assert len(read) == len(written)
                // TODO: don't unwrap
                // TODO: no clones
                let _ = file_buf.write(&buf[0..len]).await.unwrap();
                let _ = sender.send(ServerMessage::SendFileRequest((file_id.clone(), len)));
            }
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Failed to read from stream",
                ));
            }
        }
    }
    Ok(())
}
