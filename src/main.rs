use std::io;
use std::time::Duration;

use console::style;
use dialoguer::MultiSelect;
use dialoguer::{theme::ColorfulTheme, Confirm};
use localsend_core::{ClientMessage, DeviceScanner, Server, ServerMessage};
use tokio::runtime;
use tokio::sync::mpsc;
use tracing::debug;
use tracing_log::LogTracer;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

fn main() {
    init_tracing_logger();
    // TODO: should i use new_current_thread or new_multi_thread?
    let runtime = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let _ = runtime.block_on(async_main());
    // https://stackoverflow.com/questions/73528236/how-to-terminate-a-blocking-tokio-task
    // start_device_scanner blocks exit, so set timeout or use async_std crate (adds to the binary size and compile time)
    runtime.shutdown_timeout(Duration::from_millis(1));
}

async fn async_main() -> Result<(), io::Error> {
    // spawn task to listen and announce multicast messages
    start_device_scanner();

    let (server_tx, mut server_rx) = mpsc::unbounded_channel();
    let (client_tx, client_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        while let Some(server_message) = server_rx.recv().await {
            debug!("{:?}", &server_message);
            match server_message {
                ServerMessage::SendFileRequest(_) => {}
                ServerMessage::SendRequest(send_request) => {
                    println!(
                        "{} wants to send you the following files:\n",
                        style(send_request.device_info.alias).bold().magenta()
                    );

                    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select the files you want to receive")
                        .items(
                            &send_request
                                .files
                                .values()
                                .map(|file_info| file_info.file_name.as_str())
                                .collect::<Vec<&str>>(),
                        )
                        .interact()
                        .unwrap();

                    if selections.is_empty() {
                        let _ = client_tx.send(ClientMessage::Decline);
                    } else {
                        let file_ids = send_request
                            .files
                            .keys()
                            .map(|file_id| file_id.as_str())
                            .collect::<Vec<&str>>();

                        let selected_file_ids = selections
                            .into_iter()
                            .map(|idx| String::from(file_ids[idx]))
                            .collect::<Vec<_>>();
                        let _ = client_tx.send(ClientMessage::Allow(selected_file_ids));
                    }
                }
            }
        }
    });

    let server = Server::new();
    server.start_server(server_tx, client_rx).await;
    return Ok(());
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

    // forward log's from the log crate to tracing
    LogTracer::init().unwrap();
}
