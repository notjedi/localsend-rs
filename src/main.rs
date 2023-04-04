use std::io;
use std::process::exit;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use localsend_core::{ClientMessage, DeviceScanner, Server, ServerMessage};
use tokio::sync::mpsc;
use tracing::debug;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;
use tui::backend::{Backend, CrosstermBackend};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Tabs};
use tui::{Frame, Terminal};

struct App<'a> {
    pub titles: Vec<&'a str>,
    pub index: usize,
}

impl<'a> App<'a> {
    fn new() -> App<'a> {
        App {
            titles: vec!["Tab0", "Tab1", "Tab2", "Tab3"],
            index: 0,
        }
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.titles.len();
    }

    pub fn previous(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.titles.len() - 1;
        }
    }
}

#[tokio::main]
async fn main() {
    init_tracing_logger();

    // spawn task to listen and announce multicast messages
    start_device_scanner();

    let (server_tx, mut server_rx) = mpsc::unbounded_channel();
    let (client_tx, client_rx) = mpsc::unbounded_channel();

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

    tokio::spawn(async move {
        let server = Server::new();
        server.start_server(server_tx, client_rx).await;
    });

    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    // create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode().unwrap();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();
    terminal.show_cursor().unwrap();

    if let Err(err) = res {
        println!("{:?}", err);
        exit(1);
    }
    exit(0);
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('c') => {
                    if key.modifiers == KeyModifiers::CONTROL {
                        return Ok(());
                    }
                }
                KeyCode::Right => app.next(),
                KeyCode::Left => app.previous(),
                _ => {}
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(5)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(size);

    let block = Block::default().style(Style::default().bg(Color::White).fg(Color::Black));
    f.render_widget(block, size);
    let titles = app
        .titles
        .iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Spans::from(vec![
                Span::styled(first, Style::default().fg(Color::Yellow)),
                Span::styled(rest, Style::default().fg(Color::Green)),
            ])
        })
        .collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.index)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Black),
        );
    f.render_widget(tabs, chunks[0]);
    let inner = match app.index {
        0 => Block::default().title("Inner 0").borders(Borders::ALL),
        1 => Block::default().title("Inner 1").borders(Borders::ALL),
        2 => Block::default().title("Inner 2").borders(Borders::ALL),
        3 => Block::default().title("Inner 3").borders(Borders::ALL),
        _ => unreachable!(),
    };
    f.render_widget(inner, chunks[1]);
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
