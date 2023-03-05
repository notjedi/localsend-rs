use axum::{
    extract::Json,
    extract::RawBody,
    extract::RawForm,
    response::Html,
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use localsend_core::Server;
use log::{LevelFilter, Metadata, Record};
use rcgen::Certificate;
use std::net::SocketAddr;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

struct MyLogger;

impl log::Log for MyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        false
        // use log::Level;
        // metadata.level() <= Level::Info
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
    static MY_LOGGER: MyLogger = MyLogger;

    // log::set_logger(&MY_LOGGER).unwrap();
    // log::set_max_level(LevelFilter::Trace);
    // log::trace!("hi");
    // log::info!("hi");
    // log::warn!("hi");
    // log::debug!("hi");
    // log::error!("hi");

    // let subscriber = FmtSubscriber::new();
    // TODO: use env filter
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let cert = generate_tls_cert();
    let cert_pem = cert.serialize_pem().unwrap();
    let private_key_pem = cert.serialize_private_key_pem();

    let config = RustlsConfig::from_pem(cert_pem.into_bytes(), private_key_pem.into_bytes())
        .await
        .unwrap();

    // spawn task to listen and announce multicast messages
    tokio::spawn(announce_multicast());

    let app = Router::new()
        .route("/", get(handler))
        .route("/api/localsend/v1/send-request", post(send_request));

    let addr = SocketAddr::from(([192, 168, 1, 2], localsend_core::MULTICAST_PORT));
    info!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn announce_multicast() {
    let mut server = Server::new();
    server.listen_and_announce_multicast();
    // https://github.com/localsend/protocol/issues/1#issuecomment-1426998509
    // for _ in 0..localsend_core::NUM_REPEAT {
    //     server.announce_multicast(true);
    // }
}

// async fn send_request(RawForm(form): RawForm) -> &'static str {
// async fn send_request(RawBody(form): RawBody) -> &'static str {
async fn send_request(RawBody(form): RawBody) -> &'static str {
    info!("got request {:?}", form);
    r#"{"some file id": "some token",  "another file id": "some other token"}"#
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

fn generate_tls_cert() -> Certificate {
    use rcgen::{CertificateParams, DnType, DnValue};
    // let mut params: CertificateParams = Default::default();
    let mut params =
        CertificateParams::new(vec!["localsend.rs".to_string(), "localhost".to_string()]);
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
