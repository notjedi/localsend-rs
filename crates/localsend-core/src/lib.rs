use axum::{
    response::Html,
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use std::net::SocketAddr;
use std::{collections::HashMap, net::Ipv4Addr};
use tracing::{info, trace};
use uuid::Uuid;

pub mod device_scanner;
pub mod protos;
mod utils;

pub use device_scanner::*;
pub use protos::*;

pub const INTERFACE_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
pub const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
pub const MULTICAST_PORT: u16 = 53317;
const BUFFER_SIZE: u16 = 4096;
const READ_TIMEOUT: u64 = 5;

pub const NUM_REPEAT: u8 = 4;

const ALIAS: &str = "rustsend";
const DEVICE_MODEL: &str = "linux";
const DEVICE_TYPE: &str = "desktop";

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
            .route("/", get(Self::handler))
            .route("/api/localsend/v1/send-request", post(Self::send_request));

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
        // let response_json = serde_json::to_string(&wanted_files).unwrap();
        dbg!(&wanted_files);
        Json(wanted_files)
    }

    async fn handler() -> Html<&'static str> {
        Html("<h1>Hello, World!</h1>")
    }
}
