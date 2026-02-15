use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::renderer::DisplayMode;
use crate::shared::constants;

type UiTerminal = Terminal<CrosstermBackend<io::Stderr>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Splash,
    FontSize,
    Video,
    Audio,
    Render,
    Screen,
    Confirm,
}

impl Step {
    fn title(self) -> &'static str {
        match self {
            Step::Splash => "시작",
            Step::FontSize => "폰트",
            Step::Video => "영상",
            Step::Audio => "오디오",
            Step::Render => "렌더링",
            Step::Screen => "화면",
            Step::Confirm => "확인",
        }
    }

    fn progress(self) -> &'static str {
        match self {
            Step::Splash => "0/6",
            Step::FontSize => "1/6",
            Step::Video => "2/6",
            Step::Audio => "3/6",
            Step::Render => "4/6",
            Step::Screen => "5/6",
            Step::Confirm => "6/6",
        }
    }
}

pub struct MenuSelection {
    pub video_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub mode: DisplayMode,
    pub fill_screen: bool,
}

struct MenuApp {
    step: Step,
    status: String,
    should_quit: bool,
    font_input: String,
    video_files: Vec<PathBuf>,
    audio_files: Vec<PathBuf>,
    video_index: usize,
    audio_index: usize,
    render_index: usize,
    screen_index: usize,
    selection: Option<MenuSelection>,
}

impl MenuApp {
    fn load() -> Result<Self> {
        let video_files = scan_video_files()?;
        let audio_files = scan_audio_files()?;

        Ok(Self {
            step: Step::Splash,
            status: "Enter로 시작, Esc로 종료".to_string(),
            should_quit: false,
            font_input: read_current_font_size(),
            video_files,
            audio_files,
            video_index: 0,
            audio_index: 0,
            render_index: 0,
            screen_index: 0,
            selection: None,
        })
    }

    fn audio_len_with_none(&self) -> usize {
        self.audio_files.len() + 1
    }

    fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            _ => {}
        }

        match self.step {
            Step::Splash => {
                if key == KeyCode::Enter {
                    self.step = Step::FontSize;
                    self.status = "폰트 크기를 입력한 뒤 Enter를 누르세요".to_string();
                }
            }
            Step::FontSize => self.handle_font_input(key),
            Step::Video => self.handle_video_select(key),
            Step::Audio => self.handle_audio_select(key),
            Step::Render => self.handle_render_select(key),
            Step::Screen => self.handle_screen_select(key),
            Step::Confirm => self.handle_confirm(key),
        }
    }

    fn handle_font_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Backspace => {
                self.font_input.pop();
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                if c != '.' || !self.font_input.contains('.') {
                    self.font_input.push(c);
                }
            }
            KeyCode::Enter => {
                if self.font_input.trim().is_empty() {
                    self.status = "폰트 크기를 비워둘 수 없습니다".to_string();
                    return;
                }

                let parsed = self.font_input.trim().parse::<f32>();
                match parsed {
                    Ok(v) if v > 0.0 => {
                        if let Err(err) = write_font_size(&self.font_input) {
                            self.status =
                                format!("{} 저장 실패: {}", constants::GASCCI_CONFIG_FILE, err);
                            return;
                        }

                        self.status = format!(
                            "{} 폰트 크기 저장 완료: {}",
                            constants::GASCCI_CONFIG_FILE,
                            self.font_input
                        );
                        self.step = Step::Video;
                    }
                    _ => {
                        self.status = "유효한 숫자(예: 2.5)를 입력하세요".to_string();
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_video_select(&mut self, key: KeyCode) {
        if self.video_files.is_empty() {
            self.status = "assets/video 또는 assets/vidio에 재생할 영상이 없습니다".to_string();
            self.should_quit = true;
            return;
        }

        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.video_index = self.video_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.video_index + 1 < self.video_files.len() {
                    self.video_index += 1;
                }
            }
            KeyCode::Enter => {
                self.step = Step::Audio;
                self.status = "오디오 파일을 선택하세요 (없으면 오디오 없이 재생)".to_string();
            }
            _ => {}
        }
    }

    fn handle_audio_select(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.audio_index = self.audio_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.audio_index + 1 < self.audio_len_with_none() {
                    self.audio_index += 1;
                }
            }
            KeyCode::Enter => {
                self.step = Step::Render;
                self.status = "렌더링 모드를 선택하세요".to_string();
            }
            _ => {}
        }
    }

    fn handle_render_select(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.render_index = self.render_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.render_index + 1 < constants::MENU_RENDER_MODE_LABELS.len() {
                    self.render_index += 1;
                }
            }
            KeyCode::Enter => {
                self.step = Step::Screen;
                self.status = "화면 모드를 선택하세요".to_string();
            }
            _ => {}
        }
    }

    fn handle_screen_select(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.screen_index = self.screen_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.screen_index + 1 < constants::MENU_SCREEN_MODE_LABELS.len() {
                    self.screen_index += 1;
                }
            }
            KeyCode::Enter => {
                self.step = Step::Confirm;
                self.status = "Enter로 실행, Esc로 종료".to_string();
            }
            _ => {}
        }
    }

    fn handle_confirm(&mut self, key: KeyCode) {
        match key {
            KeyCode::Backspace => {
                self.step = Step::Screen;
                self.status = "화면 모드를 다시 선택하세요".to_string();
            }
            KeyCode::Enter => {
                if self.video_files.is_empty() {
                    self.status = "재생할 영상이 없습니다".to_string();
                    self.should_quit = true;
                    return;
                }

                self.selection = Some(MenuSelection {
                    video_path: self.video_files[self.video_index].clone(),
                    audio_path: if self.audio_index == 0 {
                        None
                    } else {
                        Some(self.audio_files[self.audio_index - 1].clone())
                    },
                    mode: if self.render_index == 0 {
                        DisplayMode::Rgb
                    } else {
                        DisplayMode::Ascii
                    },
                    fill_screen: self.screen_index == 0,
                });
                self.should_quit = true;
            }
            _ => {}
        }
    }
}

pub fn run_menu() -> Result<Option<MenuSelection>> {
    let mut app = MenuApp::load()?;

    if app.video_files.is_empty() {
        eprintln!("❌ assets/video (또는 assets/vidio) 디렉토리에 비디오 파일이 없습니다.");
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
    let terminal = Terminal::new(backend).context("failed to initialize terminal backend")?;

    Ok(terminal)
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

fn draw_menu(frame: &mut Frame<'_>, app: &MenuApp) {
    let area = frame.size();

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        format!(
            " {} | {} ({}) ",
            constants::APP_NAME,
            app.step.title(),
            app.step.progress()
        ),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(inner);

    draw_logo(frame, layout[0]);

    match app.step {
        Step::Splash => draw_splash(frame, layout[1]),
        Step::FontSize => draw_font_input(frame, layout[1], app),
        Step::Video => draw_video_list(frame, layout[1], app),
        Step::Audio => draw_audio_list(frame, layout[1], app),
        Step::Render => draw_render_list(frame, layout[1], app),
        Step::Screen => draw_screen_list(frame, layout[1], app),
        Step::Confirm => draw_confirm(frame, layout[1], app),
    }

    draw_footer(frame, layout[2], &app.status);
}

fn draw_logo(frame: &mut Frame<'_>, area: Rect) {
    let lines: Vec<Line<'_>> = constants::MENU_LOGO
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                *line,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    let logo = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(logo, area);
}

fn draw_splash(frame: &mut Frame<'_>, area: Rect) {
    let content = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Ratatui 설정 메뉴",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Enter: 시작"),
        Line::from("Esc / q: 종료"),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });

    frame.render_widget(content, area);
}

fn draw_font_input(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title("폰트 크기 입력 (예: 2.5)");

    let text = if app.font_input.is_empty() {
        "_".to_string()
    } else {
        format!("{}_", app.font_input)
    };

    let input = Paragraph::new(text)
        .block(input_block)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(input, area);
}

fn draw_video_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let items: Vec<ListItem<'_>> = app
        .video_files
        .iter()
        .map(|path| {
            ListItem::new(Line::from(
                path.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string()),
            ))
        })
        .collect();

    draw_select_list(frame, area, "영상 선택", items, app.video_index);
}

fn draw_audio_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let mut items = vec![ListItem::new(constants::MENU_NO_AUDIO_LABEL)];
    items.extend(app.audio_files.iter().map(|path| {
        ListItem::new(
            path.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string()),
        )
    }));

    draw_select_list(frame, area, "오디오 선택", items, app.audio_index);
}

fn draw_render_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let items = constants::MENU_RENDER_MODE_LABELS
        .iter()
        .map(|item| ListItem::new(*item))
        .collect::<Vec<_>>();

    draw_select_list(frame, area, "렌더링 모드", items, app.render_index);
}

fn draw_screen_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let items = constants::MENU_SCREEN_MODE_LABELS
        .iter()
        .map(|item| ListItem::new(*item))
        .collect::<Vec<_>>();

    draw_select_list(frame, area, "화면 모드", items, app.screen_index);
}

fn draw_confirm(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let video = app.video_files[app.video_index]
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            app.video_files[app.video_index]
                .to_string_lossy()
                .to_string()
        });

    let audio = if app.audio_index == 0 {
        constants::MENU_NO_AUDIO_LABEL.to_string()
    } else {
        app.audio_files[app.audio_index - 1]
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                app.audio_files[app.audio_index - 1]
                    .to_string_lossy()
                    .to_string()
            })
    };

    let mode = if app.render_index == 0 {
        "rgb"
    } else {
        "ascii"
    };
    let fill_screen = if app.screen_index == 0 {
        "true"
    } else {
        "false"
    };

    let confirm = Paragraph::new(vec![
        Line::from(Span::styled(
            "실행 설정 확인",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Video: {}", video)),
        Line::from(format!("Audio: {}", audio)),
        Line::from(format!("Mode: {}", mode)),
        Line::from(format!("Fill: {}", fill_screen)),
        Line::from(""),
        Line::from("Enter: 실행   Backspace: 이전   Esc: 종료"),
    ])
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(confirm, area);
}

fn draw_select_list(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    items: Vec<ListItem<'_>>,
    selected: usize,
) {
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, status: &str) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[↑↓/j,k] 이동  [Enter] 선택  [Esc/q] 종료  ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(status, Style::default().fg(Color::White)),
    ]))
    .alignment(Alignment::Left)
    .wrap(Wrap { trim: true });

    frame.render_widget(footer, area);
}

fn scan_video_files() -> Result<Vec<PathBuf>> {
    let Some(video_dir) = constants::VIDEO_DIR_CANDIDATES
        .iter()
        .map(Path::new)
        .find(|dir| dir.exists())
    else {
        return Ok(Vec::new());
    };

    let mut video_files: Vec<PathBuf> = fs::read_dir(video_dir)
        .with_context(|| format!("failed to read {}", video_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| has_allowed_extension(path, constants::VIDEO_EXTENSIONS))
        .collect();

    video_files.sort();
    Ok(video_files)
}

fn scan_audio_files() -> Result<Vec<PathBuf>> {
    let audio_dir = Path::new(constants::AUDIO_DIR);
    if !audio_dir.exists() {
        return Ok(Vec::new());
    }

    let mut audio_files: Vec<PathBuf> = fs::read_dir(audio_dir)
        .with_context(|| format!("failed to read {}", audio_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| has_allowed_extension(path, constants::AUDIO_EXTENSIONS))
        .collect();

    audio_files.sort();
    Ok(audio_files)
}

fn has_allowed_extension(path: &Path, allowed: &[&str]) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };

    let ext = ext.to_ascii_lowercase();
    allowed
        .iter()
        .any(|allowed_ext| *allowed_ext == ext.as_str())
}

fn read_current_font_size() -> String {
    let config_path = Path::new(constants::GASCCI_CONFIG_FILE);

    if let Ok(content) = fs::read_to_string(config_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("font-size") {
                if let Some(value) = trimmed.split('=').nth(1) {
                    return value.trim().to_string();
                }
            }
        }
    }

    constants::DEFAULT_FONT_SIZE.to_string()
}

fn write_font_size(font_size: &str) -> Result<()> {
    let config_path = Path::new(constants::GASCCI_CONFIG_FILE);

    let mut replaced = false;
    let mut lines = if let Ok(content) = fs::read_to_string(config_path) {
        content
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("font-size") {
                    replaced = true;
                    format!("font-size = {}", font_size.trim())
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if !replaced {
        lines.push(format!("font-size = {}", font_size.trim()));
    }

    let mut new_content = lines.join("\n");
    new_content.push('\n');

    fs::write(config_path, new_content)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    Ok(())
}
