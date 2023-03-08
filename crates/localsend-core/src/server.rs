use crate::{utils, SendInfo};
use axum::{
    body::Bytes,
    extract::{BodyStream, Query},
    routing::post,
    BoxError, Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use futures::{Stream, TryStreamExt};
use std::collections::HashMap;
use std::{io, net::SocketAddr};
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;
use tracing::{info, trace};
use uuid::Uuid;

pub struct Server {
    certificate: rcgen::Certificate,
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

        let app = Router::new()
            .route("/api/localsend/v1/send-request", post(Self::send_request))
            .route("/api/localsend/v1/send", post(Self::incoming_send_post));

        let addr = SocketAddr::from(([0, 0, 0, 0], crate::MULTICAST_PORT));
        info!("listening on {}", addr);
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }

    async fn send_request(
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

    async fn incoming_send_post(params: Query<SendInfo>, file_stream: BodyStream) {
        stream_to_file("hi", file_stream).await;
        // dbg!(&file_stream);
        // let body_bytes = hyper::body::to_bytes(body).await;
        // Ok(String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8"))
    }
}

async fn stream_to_file<S, E>(path: &str, stream: S)
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    // if !path_is_valid(path) {
    //     return Err((StatusCode::BAD_REQUEST, "Invalid path".to_owned()));
    // }

    // Convert the stream into an `AsyncRead`.
    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
    let body_reader = StreamReader::new(body_with_io_error);
    futures::pin_mut!(body_reader);

    // Create the file. `File` implements `AsyncWrite`.
    // let path = std::path::Path::new(UPLOADS_DIRECTORY).join(path);
    // let mut file = BufWriter::new(File::create(path).await);

    let mut file = BufWriter::new(tokio::io::stdout());

    // Copy the body into the file.
    tokio::io::copy(&mut body_reader, &mut file).await.unwrap();
}
