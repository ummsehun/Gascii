use super::menu_draw::draw_menu;
use super::menu_state::MenuApp;
pub use super::menu_state::MenuSelection;
use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

type UiTerminal = Terminal<CrosstermBackend<io::Stderr>>;

pub fn run_menu() -> Result<Option<MenuSelection>> {
    let mut app = MenuApp::load()?;

    if app.video_files.is_empty() {
        let assets_dir = crate::utils::runtime::detect().assets_dir;
        let message = format!(
            "{} 아래 video (또는 vidio) 디렉토리에 비디오 파일이 없습니다.",
            assets_dir.display()
        );
        crate::utils::logger::error(&message);
        eprintln!("{}", message);
        return Ok(None);
    }

    let mut terminal = setup_terminal()?;
    let run_result = run_app(&mut terminal, &mut app);
    let restore_result = restore_terminal(&mut terminal);

    if let Err(err) = restore_result {
        crate::utils::logger::error(&format!("Failed to restore terminal from menu: {}", err));
    }

    run_result?;
    Ok(app.selection)
}

fn setup_terminal() -> Result<UiTerminal> {
    enable_raw_mode().context("failed to enable raw mode")?;

    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, Hide).context("failed to switch to alternate screen")?;

    let backend = CrosstermBackend::new(stderr);
    Terminal::new(backend).context("failed to initialize terminal backend")
}

fn restore_terminal(terminal: &mut UiTerminal) -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;
    Ok(())
}

fn run_app(terminal: &mut UiTerminal, app: &mut MenuApp) -> Result<()> {
    loop {
        terminal.draw(|frame| draw_menu(frame, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key.code);
                }
            }
        }
    }

    Ok(())
}
