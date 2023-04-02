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

type ReceiveState = Arc<Mutex<ReceiveSession>>;

#[derive(Clone, PartialEq)]
#[allow(unused)]
pub enum ReceiveStatus {
    Idle,               // no ongoing session
    Waiting,            // wait for sender to send the files
    Receiving,          // in an ongoing session, receiving files
    Finished,           // all files received (end of session)
    FinishedWithErrors, // finished but some files could not be received (end of session)
}
// CanceledBySender,   // cancellation by sender  (end of session)
// CanceledByReceiver, // cancellation by receiver (end of session)

#[derive(Clone)]
pub struct ReceiveSession {
    pub sender: DeviceInfo,
    pub files: HashMap<String, FileInfo>,
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
            start_time: Instant::now(),
            status: ReceiveStatus::Idle,
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
        let session_state: ReceiveState = Arc::new(Mutex::new(ReceiveSession::new(
            DeviceInfo::new(),
            "".into(),
        )));

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

        let mut wanted_files: HashMap<String, String> = HashMap::new();
        let mut state = session_state.lock().unwrap();

        if state.status != ReceiveStatus::Idle {
            // reject incoming request if another session is ongoing
            return Err((StatusCode::CONFLICT, "Blocked by another sesssion".into()));
        } else {
            state.sender = send_request.device_info;
            state.status = ReceiveStatus::Waiting;
            state.destination_directory = "/home/jedi".into();
        }

        send_request
            .files
            .into_iter()
            .for_each(|(file_id, file_info)| {
                let token = Uuid::new_v4();
                wanted_files.insert(file_id.clone(), token.to_string());
                state.files.insert(file_id, file_info);
            });
        trace!("{:#?}", wanted_files);
        dbg!(&state.files);
        Ok(Json(wanted_files))
    }

    async fn incoming_send_post(
        State(session_state): State<ReceiveState>,
        params: Query<SendInfo>,
        file_stream: BodyStream,
    ) {
        trace!("{:?}", &params);

        // https://users.rust-lang.org/t/strange-compiler-error-bug-axum-handler/71352/3
        let file_name = {
            let state = session_state.lock().unwrap();
            state.files[&params.file_id].file_name.clone()
        };
        dbg!(&file_name);
        stream_to_file(file_name.as_str(), file_stream).await;
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
