use std::{collections::HashMap, fmt::Write, io, net::Ipv4Addr, time::Duration};

use console::style;
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use tokio::{runtime, sync::mpsc};
use tracing::{debug, info};
use tracing_subscriber::{filter::EnvFilter, FmtSubscriber};

use localsend_core::{ClientMessage, DeviceScanner, FileInfo, Server, ServerMessage};

const ALIAS: &str = "rustsend";
const INTERFACE_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
const MULTICAST_PORT: u16 = 53317;

struct State {
    multi_progress: MultiProgress,
    files: HashMap<String, FileInfo>,
    progress_map: HashMap<String, ProgressBar>,
}

fn main() {
    console_subscriber::init();
    // init_tracing_logger();
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

async fn handle_server_msgs(
    mut server_rx: localsend_core::protos::Receiver<ServerMessage>,
    client_tx: localsend_core::protos::Sender<ClientMessage>,
) {
    // TODO: set this back to None when we are done with a session
    let mut client_state: Option<State> = None;

    while let Some(server_message) = server_rx.recv().await {
        debug!("{:?}", &server_message);
        match server_message {
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
                    .defaults(vec![true; send_request.files.len()].as_slice())
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

                    let multi_progress = MultiProgress::new();
                    let progress_map = send_request
                            .files
                            .clone()
                            .into_iter()
                            .map(|(file_id, file_info)| {
                                // TODO(notjedi): change length ot size of file
                                let pb =
                                    multi_progress.add(ProgressBar::new(file_info.size as u64));

                                pb.set_style(ProgressStyle::with_template("{spinner:.green} [{msg}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                                    .unwrap()
                                    .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
                                    .progress_chars("#>-"));

                                pb.set_message(file_info.file_name);
                                (file_id, pb)
                            })
                            .collect::<HashMap<String, ProgressBar>>();

                    client_state = Some(State {
                        files: send_request.files,
                        multi_progress,
                        progress_map,
                    });
                }
            }
            ServerMessage::SendFileRequest((file_id, size)) => match client_state.as_ref() {
                Some(state) => {
                    state.progress_map[&file_id].inc(size as u64);
                    if state.progress_map[&file_id].position()
                        == (state.files[&file_id].size as u64)
                    {
                        state.progress_map[&file_id].finish_and_clear();
                        state
                            .multi_progress
                            .println(format!("Received {}", state.files[&file_id].file_name))
                            .unwrap();
                    }
                }
                None => {
                    info!("client_state is None. this shouldn't be happening as this block is unreachable.")
                }
            },
            ServerMessage::CancelSession => match client_state.as_ref() {
                // TODO(notjedi): handle cancel request when in send request phase
                Some(state) => {
                    for (file_id, pb) in &state.progress_map {
                        if !pb.is_finished() {
                            pb.finish_and_clear();
                            state
                                .multi_progress
                                .println(format!(
                                    "{} finished with error",
                                    state.files[file_id.as_str()].file_name
                                ))
                                .unwrap();
                        }
                    }
                    client_state = None;
                }
                None => {
                    info!("client_state is None. this shouldn't be happening as this block is unreachable.")
                }
            },
        }
    }
}

async fn start_device_scanner() {
    let mut device_scanner = DeviceScanner::new(
        ALIAS.to_string(),
        INTERFACE_ADDR,
        MULTICAST_ADDR,
        MULTICAST_PORT,
    )
    .await;
    device_scanner.listen_and_announce_multicast().await;
}

async fn async_main() -> Result<(), io::Error> {
    // spawn task to listen and announce multicast messages
    tokio::spawn(start_device_scanner());

    let (server_tx, server_rx) = mpsc::unbounded_channel();
    let (client_tx, client_rx) = mpsc::unbounded_channel();

    tokio::spawn(handle_server_msgs(server_rx, client_tx));

    let server = Server::new(INTERFACE_ADDR, MULTICAST_PORT);
    server.start_server(server_tx, client_rx).await;
    Ok(())
}

fn init_tracing_logger() {
    let mut subscriber_builder = FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .without_time();

    if cfg!(debug_assertions) {
        subscriber_builder = subscriber_builder.with_line_number(true);
    }
    let subscriber = subscriber_builder.finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // forward log's from the log crate to tracing
    #[cfg(debug_assertions)]
    {
        use tracing_log::LogTracer;
        LogTracer::builder()
            .with_max_level(tracing_log::log::LevelFilter::Off)
            .init()
            .unwrap();
    }
}
