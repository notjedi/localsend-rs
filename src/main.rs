use axum::{
    extract::RawForm,
    response::Html,
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use localsend_core::Server;
use rcgen::Certificate;
use std::net::SocketAddr;
use tracing::info;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    // let subscriber = FmtSubscriber::new();
    // TODO: use env filter
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
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

    let addr = SocketAddr::from((
        localsend_core::INTERFACE_ADDR,
        localsend_core::MULTICAST_PORT,
    ));
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

async fn send_request(RawForm(form): RawForm) -> Html<&'static str> {
    info!("got request {:?}", form);
    Html("<h1>Hello from send-request</h1>")
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

fn generate_tls_cert() -> Certificate {
    use rcgen::{CertificateParams, DnType, DnValue};
    let mut params: CertificateParams = Default::default();
    params.distinguished_name.push(
        DnType::CommonName,
        DnValue::PrintableString("Localsend client".to_string()),
    );
    Certificate::from_params(params).unwrap()
}
