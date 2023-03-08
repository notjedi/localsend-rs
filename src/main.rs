use localsend_core::{DeviceScanner, Server};
use log::{Metadata, Record};
use tracing::level_filters::LevelFilter;
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
