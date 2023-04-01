use crate::{utils, DeviceResponse, FileInfo, SendInfo};
use axum::{
    body::Bytes,
    extract::{BodyStream, Query, State},
    routing::post,
    BoxError, Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use futures::{Stream, TryStreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use std::{io, net::SocketAddr};
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;
use tracing::{info, trace};
use uuid::Uuid;

#[derive(Clone)]
enum SessionStatus {
    Waiting,            // wait for receiver response (wait for decline / accept)
    RecipientBusy,      // recipient is busy with another request (end of session)
    Declined,           // receiver declined the request (end of session)
    Sending,            // files are being sent
    Finished,           // all files sent (end of session)
    FinishedWithErrors, // finished but some files could not be sent (end of session)
    CanceledBySender,   // cancellation by sender  (end of session)
    CanceledByReceiver, // cancellation by receiver (end of session)
}

#[derive(Clone)]
pub struct ReceiveSession {
    sender: DeviceResponse,
    files: HashMap<String, FileInfo>,
    destination_directory: String,
    start_time: Instant,
    status: SessionStatus,
}

impl ReceiveSession {
    fn new(
        sender: DeviceResponse,
        files: HashMap<String, FileInfo>,
        destination_directory: String,
    ) -> Self {
        Self {
            sender,
            files,
            destination_directory,
            start_time: Instant::now(),
            status: SessionStatus::Sending,
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
        let session_state: Arc<Option<ReceiveSession>> = Arc::new(None);

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
        State(session_state): State<Arc<Option<ReceiveSession>>>,
        Json(send_request): Json<crate::SendRequest>,
    ) -> Json<HashMap<String, String>> {
        trace!("got request {:#?}", send_request);

        let mut wanted_files: HashMap<String, String> = HashMap::new();
        send_request.files.into_iter().for_each(|(file_id, _)| {
            let token = Uuid::new_v4();
            wanted_files.insert(file_id, token.to_string());
        });
        trace!("{:#?}", wanted_files);
        Json(wanted_files)
    }

    async fn incoming_send_post(
        params: Query<SendInfo>,
        State(session_state): State<Arc<Option<ReceiveSession>>>,
        file_stream: BodyStream,
    ) {
        trace!("{:?}", &params);
        stream_to_file("hi", file_stream).await;
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
