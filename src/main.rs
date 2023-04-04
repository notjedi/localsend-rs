use localsend_core::{ClientMessage, DeviceScanner, Server, ServerMessage};
use tokio::sync::mpsc;
use tracing::debug;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    init_tracing_logger();

    // spawn task to listen and announce multicast messages
    start_device_scanner();

    let (server_tx, mut server_rx) = mpsc::unbounded_channel();
    let (client_tx, client_rx) = mpsc::unbounded_channel();
    let server = Server::new();

    tokio::spawn(async move {
        while let Some(server_message) = server_rx.recv().await {
            debug!("{:?}", &server_message);
            match server_message {
                ServerMessage::SendFileRequest => {}
                ServerMessage::SendRequest => {
                    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
                    let mut stdout = tokio::io::stdout();
                    let _ = stdout
                        .write_all(
                            format!(
                                "Do you want to accept the send request from notjedi [y/n]? ",
                                // send_request.device_info.alias
                            )
                            .as_bytes(),
                        )
                        .await;
                    let _ = stdout.flush().await;

                    let mut buf = Vec::new();
                    let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
                    let _ = reader.read_until(b'\n', &mut buf).await;
                    let input = std::str::from_utf8(&buf).unwrap();
                    let input = input.trim();

                    if input != "y" && input != "Y" {
                        let _ = client_tx.send(ClientMessage::Decline);
                    } else {
                        let _ = client_tx.send(ClientMessage::Allow);
                    }
                }
            }
        }
    });

    server.start_server(server_tx, client_rx).await;
}

fn start_device_scanner() {
    // NOTE: https://ryhl.io/blog/async-what-is-blocking recommends that we run functions that run
    // forever in a separate thread.
    tokio::task::spawn_blocking(|| {
        let mut server = DeviceScanner::new();
        server.announce_multicast_repeated();
        server.listen_and_announce_multicast();
    });
}

fn init_tracing_logger() {
    let subscriber = FmtSubscriber::builder()
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
