mod app;
mod jakim;
mod ui;

use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::{Action, App, AppEvent, SearchMode};
use jakim::JakimClient;

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = Arc::new(JakimClient::new()?);
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    let mut terminal = init_terminal()?;
    let mut app = App::new();

    let result = run(&mut terminal, &mut app, client, tx, &mut rx).await;

    restore_terminal()?;
    terminal.show_cursor()?;

    if let Err(err) = &result {
        eprintln!("cekhalal exited with an error: {err:#}");
    }
    result
}

async fn run(
    terminal: &mut Tui,
    app: &mut App,
    client: Arc<JakimClient>,
    tx: mpsc::UnboundedSender<AppEvent>,
    rx: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let action = app.handle_key(key);
                    dispatch(action, app, &client, &tx);
                }
            }
        }

        while let Ok(evt) = rx.try_recv() {
            app.apply_event(evt);
        }

        if let Some((comp_code, type_, ty)) = app.pending_preview.take() {
            let client = client.clone();
            let tx = tx.clone();
            tokio::spawn(async move {
                let res = client.detail(&comp_code, &type_, &ty).await;
                let _ = tx.send(AppEvent::DetailResult(comp_code, res));
            });
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn dispatch(action: Action, app: &mut App, client: &Arc<JakimClient>, tx: &mpsc::UnboundedSender<AppEvent>) {
    match action {
        Action::None => {}
        Action::Quit => app.should_quit = true,
        Action::RunSearch => {
            app.loading = true;
            app.error = None;
            let keyword = app.input.clone();
            let state = app.state_code().to_string();
            let page = app.page;
            let client = client.clone();
            let tx = tx.clone();
            match app.search_mode {
                SearchMode::Company => {
                    let category = app.category_code().to_string();
                    tokio::spawn(async move {
                        let res = client.search(&keyword, &state, &category, page).await;
                        let _ = tx.send(AppEvent::SearchResult(res));
                    });
                }
                SearchMode::Product => {
                    tokio::spawn(async move {
                        let res = client.search_products(&keyword, &state, page).await;
                        let _ = tx.send(AppEvent::SearchResult(res));
                    });
                }
            }
        }
    }
}
