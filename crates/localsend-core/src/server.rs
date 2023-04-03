use crate::{utils, DeviceInfo, FileInfo, SendInfo};
use axum::{
    body::Bytes,
    extract::{BodyStream, Query, State},
    http::StatusCode,
    routing::post,
    BoxError, Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use futures::{Stream, TryStreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{io, net::SocketAddr};
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;
use tracing::{info, trace};
use uuid::Uuid;

type ReceiveState = Arc<Mutex<Option<ReceiveSession>>>;

#[derive(Clone, PartialEq, Debug)]
pub enum ReceiveStatus {
    // TODO: add status for cancelled
    Waiting,            // waiting for sender to send the files
    Receiving,          // in an ongoing session, receiving files
    Finished,           // all files received (end of session)
    FinishedWithErrors, // finished but some files could not be received (end of session)
}

#[derive(Clone)]
pub struct ReceiveSession {
    pub sender: DeviceInfo,
    pub files: HashMap<String, FileInfo>,
    pub file_status: HashMap<String, ReceiveStatus>,
    pub destination_directory: String,
    pub start_time: Instant,
    pub status: ReceiveStatus,
}

impl ReceiveSession {
    fn new(sender: DeviceInfo, destination_directory: String) -> Self {
        Self {
            sender,
            destination_directory,
            files: HashMap::new(),
            file_status: HashMap::new(),
            start_time: Instant::now(),
            status: ReceiveStatus::Waiting,
        }
    }
}

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

    pub async fn start_server(&self) {
        let cert_pem = self.certificate.serialize_pem().unwrap();
        let private_key_pem = self.certificate.serialize_private_key_pem();
        let config = RustlsConfig::from_pem(cert_pem.into_bytes(), private_key_pem.into_bytes())
            .await
            .unwrap();
        let session_state = Arc::new(Mutex::new(None));

        let app = Router::new()
            .route("/api/localsend/v1/send-request", post(Self::send_request))
            .route("/api/localsend/v1/send", post(Self::incoming_send_post))
            .with_state(session_state);

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

        let mut session = session_state.lock().unwrap();
        if session.is_some() {
            // reject incoming request if another session is ongoing
            return Err((StatusCode::CONFLICT, "Blocked by another sesssion".into()));
        }
        trace!("session_state is None");

        let mut state = session.insert(ReceiveSession::new(DeviceInfo::default(), "".into()));
        state.sender = send_request.device_info;
        state.status = ReceiveStatus::Waiting;
        state.destination_directory = "/home/jedi".into();

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
        trace!("{:#?}", wanted_files);
        trace!("{:#?}, ", &state.files);
        Ok(Json(wanted_files))
    }

    // #[axum_macros::debug_handler]
    async fn incoming_send_post(
        State(session_state): State<ReceiveState>,
        params: Query<SendInfo>,
        file_stream: BodyStream,
    ) -> Result<(), (StatusCode, String)> {
        trace!("{:?}", &params);

        let (file_id, file_info) = {
            // https://users.rust-lang.org/t/strange-compiler-error-bug-axum-handler/71352/3
            // https://github.com/tokio-rs/axum/discussions/641
            let mut session = session_state.lock().unwrap();
            if session.is_none() {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Call to /send without requesting a send".into(),
                ));
            }
            let mut state = session.as_mut().unwrap();
            state.status = ReceiveStatus::Receiving;
            (params.file_id.clone(), state.files[&params.file_id].clone())
        };

        // TODO: catch erros in this method
        stream_to_file(file_info.file_name.as_str(), file_stream).await;

        let mut session = session_state.lock().unwrap();
        let all_finished = {
            let mut state = session.as_mut().unwrap();
            state
                .file_status
                .entry(file_id)
                .and_modify(|file_status| *file_status = ReceiveStatus::Finished);
            let all_finished = state.files.iter().all(|(file_status_id, _)| {
                state.file_status[file_status_id] == ReceiveStatus::Finished
                    || state.file_status[file_status_id] == ReceiveStatus::FinishedWithErrors
            });
            if !all_finished {
                state.status = ReceiveStatus::Receiving;
            }
            all_finished
        };

        if all_finished {
            *session = None;
        }
        return Ok(());
    }
}

// from: https://github.com/tokio-rs/axum/blob/main/examples/stream-to-file/src/main.rs
async fn stream_to_file<S, E>(path: &str, stream: S)
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>, // BoxError is just - Box<dyn std::error::Error + Send + Sync>
{
    // Convert the stream into an `AsyncRead`.
    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
    let body_reader = StreamReader::new(body_with_io_error);
    futures::pin_mut!(body_reader);

    // Create the file. `File` implements `AsyncWrite`.
    let path = std::path::Path::new(path);
    let mut file = BufWriter::new(File::create(path).await.unwrap());

    // Copy the body into the file.
    tokio::io::copy(&mut body_reader, &mut file).await.unwrap();
}
