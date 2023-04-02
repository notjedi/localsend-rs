use localsend_core::{DeviceScanner, Server};
use tracing_subscriber::{fmt::time::UtcTime, EnvFilter, FmtSubscriber};

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
    let time_format = time::format_description::parse("[hour]:[minute]:[second]")
        .expect("format string should be valid!");
    let timer = UtcTime::new(time_format);

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .with_line_number(true)
        .with_timer(timer)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}
