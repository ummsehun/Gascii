use super::menu_state::{display_name, MenuApp, Step};
use crate::core::player::{RenderQuality, ViewportMode};
use crate::renderer::DisplayMode;
use crate::shared::constants;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use std::path::PathBuf;

pub(super) fn draw_menu(frame: &mut Frame<'_>, app: &MenuApp) {
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
        Step::Video => draw_video_list(frame, layout[1], app),
        Step::Audio => draw_audio_list(frame, layout[1], app),
        Step::Render => draw_render_list(frame, layout[1], app),
        Step::Quality => draw_quality_list(frame, layout[1], app),
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
            "Gascii 설정 메뉴",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("폰트 설정 단계는 제거되었습니다."),
        Line::from("터미널 Zoom In/Out 또는 창 크기 변경 시 재생 중 자동으로 다시 맞춰집니다."),
        Line::from(""),
        Line::from("Enter: 시작"),
        Line::from("Esc / q: 종료"),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });

    frame.render_widget(content, area);
}

fn draw_video_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let items = app
        .video_files
        .iter()
        .map(path_to_list_item)
        .collect::<Vec<_>>();
    draw_select_list(frame, area, "영상 선택", items, app.video_index);
}

fn draw_audio_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let mut items = vec![ListItem::new(constants::MENU_NO_AUDIO_LABEL)];
    items.extend(app.audio_files.iter().map(path_to_list_item));
    draw_select_list(frame, area, "오디오 선택", items, app.audio_index);
}

fn draw_render_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let items = constants::MENU_RENDER_MODE_LABELS
        .iter()
        .map(|item| ListItem::new(*item))
        .collect::<Vec<_>>();
    draw_select_list(frame, area, "렌더링 모드", items, app.render_index);
}

fn draw_quality_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let items = constants::MENU_QUALITY_LABELS
        .iter()
        .map(|item| ListItem::new(*item))
        .collect::<Vec<_>>();
    draw_select_list(frame, area, "품질", items, app.quality_index);
}

fn draw_screen_list(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let items = constants::MENU_SCREEN_MODE_LABELS
        .iter()
        .map(|item| ListItem::new(*item))
        .collect::<Vec<_>>();
    draw_select_list(frame, area, "화면 모드", items, app.screen_index);
}

fn draw_confirm(frame: &mut Frame<'_>, area: Rect, app: &MenuApp) {
    let video = display_name(&app.video_files[app.video_index]);
    let audio = if app.audio_index == 0 {
        constants::MENU_NO_AUDIO_LABEL.to_string()
    } else {
        display_name(&app.audio_files[app.audio_index - 1])
    };

    let mode = match app.selected_display_mode() {
        DisplayMode::Rgb => "RGB",
        DisplayMode::Ascii => "ASCII",
    };
    let viewport = match app.selected_viewport_mode() {
        ViewportMode::Fullscreen => "전체 화면",
        ViewportMode::CinemaScope => "시네마스코프 (2.39:1)",
    };
    let quality = match app.selected_quality() {
        RenderQuality::Full => "Full",
        RenderQuality::Balanced => "Balanced",
        RenderQuality::Performance => "Performance",
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
        Line::from(format!("Quality: {}", quality)),
        Line::from(format!("Viewport: {}", viewport)),
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
            "[↑↓/j,k] 이동  [Enter] 선택  [Backspace] 이전  [Esc/q] 종료  ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(status, Style::default().fg(Color::White)),
    ]))
    .alignment(Alignment::Left)
    .wrap(Wrap { trim: true });

    frame.render_widget(footer, area);
}

fn path_to_list_item(path: &PathBuf) -> ListItem<'_> {
    ListItem::new(display_name(path))
}
