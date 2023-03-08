use axum::{
    response::Html,
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use localsend_core::DeviceScanner;
use log::{Metadata, Record};
use rcgen::Certificate;
use std::net::SocketAddr;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::FmtSubscriber;

struct MyLogger;

impl log::Log for MyLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        false
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!(
                "{:?}:{:?} - {} - {}",
                record.file(),
                record.line(),
                record.level(),
                record.args()
            );
        }
    }
    fn flush(&self) {}
}

#[tokio::main]
async fn main() {
    init_logger(log::LevelFilter::Debug);
    init_tracing_logger(LevelFilter::DEBUG);

    let cert = generate_tls_cert();
    let cert_pem = cert.serialize_pem().unwrap();
    let private_key_pem = cert.serialize_private_key_pem();

    let config = RustlsConfig::from_pem(cert_pem.into_bytes(), private_key_pem.into_bytes())
        .await
        .unwrap();

    // spawn task to listen and announce multicast messages
    start_device_scanner();

    let app = Router::new()
        .route("/", get(handler))
        .route("/api/localsend/v1/send-request", post(send_request));

    let addr = SocketAddr::from(([0, 0, 0, 0], localsend_core::MULTICAST_PORT));
    info!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn start_device_scanner() {
    // NOTE: https://ryhl.io/blog/async-what-is-blocking recommends that we run functions that run
    // forever in a separate thread.
    tokio::task::spawn_blocking(|| {
        let mut server = DeviceScanner::new();
        server.announce_multicast_repeated();
        server.listen_and_announce_multicast();
    });
    // std::thread::sleep(std::time::Duration::from_secs(5));
}

async fn send_request(Json(send_request): Json<localsend_core::SendRequest>) -> &'static str {
    info!("got request {:#?}", send_request);
    r#"{"some file id": "some token",  "another file id": "some other token"}"#
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

fn init_tracing_logger(level: LevelFilter) {
    // TODO: use env filter
    // let subscriber = FmtSubscriber::new();
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

fn init_logger(level: log::LevelFilter) {
    static MY_LOGGER: MyLogger = MyLogger;
    log::set_logger(&MY_LOGGER).unwrap();
    log::set_max_level(level);
}

fn generate_tls_cert() -> Certificate {
    use rcgen::{CertificateParams, DnType, DnValue};
    let mut params: CertificateParams = Default::default();
    // TODO: can we do `From` hashmap
    params.distinguished_name.push(
        DnType::CommonName,
        DnValue::PrintableString("Localsend client".to_string()),
    );
    params
        .distinguished_name
        .push(DnType::OrganizationName, "".to_string());
    params
        .distinguished_name
        .push(DnType::OrganizationalUnitName, "".to_string());
    params
        .distinguished_name
        .push(DnType::LocalityName, "".to_string());
    params
        .distinguished_name
        .push(DnType::StateOrProvinceName, "".to_string());
    params
        .distinguished_name
        .push(DnType::CountryName, "".to_string());
    Certificate::from_params(params).unwrap()
}
