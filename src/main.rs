use localsend_core::{DeviceScanner, Server};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    init_tracing_logger();

    // spawn task to listen and announce multicast messages
    start_device_scanner();

    let server = Server::new();
    server.start_server().await;
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

fn init_tracing_logger() {
    // TODO: use env filter

    let subscriber = FmtSubscriber::builder()
        // .with_env_filter(EnvFilter::from_default_env())
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_line_number(true)
        .without_time()
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}
