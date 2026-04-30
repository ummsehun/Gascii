use crate::core::player::{RenderQuality, ViewportMode};
use crate::renderer::DisplayMode;
use crate::shared::constants;
use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Splash,
    Video,
    Audio,
    Render,
    Quality,
    Screen,
    Confirm,
}

impl Step {
    pub(super) fn title(self) -> &'static str {
        match self {
            Step::Splash => "시작",
            Step::Video => "영상",
            Step::Audio => "오디오",
            Step::Render => "렌더링",
            Step::Quality => "품질",
            Step::Screen => "화면",
            Step::Confirm => "확인",
        }
    }

    pub(super) fn progress(self) -> &'static str {
        match self {
            Step::Splash => "0/6",
            Step::Video => "1/6",
            Step::Audio => "2/6",
            Step::Render => "3/6",
            Step::Quality => "4/6",
            Step::Screen => "5/6",
            Step::Confirm => "6/6",
        }
    }
}

pub struct MenuSelection {
    pub video_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub display_mode: DisplayMode,
    pub viewport_mode: ViewportMode,
    pub quality: RenderQuality,
}

pub(super) struct MenuApp {
    pub(super) step: Step,
    pub(super) status: String,
    pub(super) should_quit: bool,
    pub(super) video_files: Vec<PathBuf>,
    pub(super) audio_files: Vec<PathBuf>,
    pub(super) video_index: usize,
    pub(super) audio_index: usize,
    pub(super) render_index: usize,
    pub(super) quality_index: usize,
    pub(super) screen_index: usize,
    pub(super) selection: Option<MenuSelection>,
}

impl MenuApp {
    pub(super) fn load() -> Result<Self> {
        Ok(Self {
            step: Step::Splash,
            status: "Enter로 시작, Esc로 종료".to_string(),
            should_quit: false,
            video_files: scan_video_files()?,
            audio_files: scan_audio_files()?,
            video_index: 0,
            audio_index: 0,
            render_index: 0,
            quality_index: 0,
            screen_index: 0,
            selection: None,
        })
    }

    pub(super) fn audio_len_with_none(&self) -> usize {
        self.audio_files.len() + 1
    }

    pub(super) fn on_key(&mut self, key: KeyCode) {
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
                    self.step = Step::Video;
                    self.status = "재생할 비디오를 선택하세요".to_string();
                }
            }
            Step::Video => self.handle_video_select(key),
            Step::Audio => self.handle_audio_select(key),
            Step::Render => self.handle_render_select(key),
            Step::Quality => self.handle_quality_select(key),
            Step::Screen => self.handle_screen_select(key),
            Step::Confirm => self.handle_confirm(key),
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
                self.step = Step::Quality;
                self.status = "품질 정책을 선택하세요".to_string();
            }
            _ => {}
        }
    }

    fn handle_quality_select(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.quality_index = self.quality_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.quality_index + 1 < constants::MENU_QUALITY_LABELS.len() {
                    self.quality_index += 1;
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
                self.status = "Enter로 실행, Backspace로 이전 단계".to_string();
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
                self.selection = Some(MenuSelection {
                    video_path: self.video_files[self.video_index].clone(),
                    audio_path: if self.audio_index == 0 {
                        None
                    } else {
                        Some(self.audio_files[self.audio_index - 1].clone())
                    },
                    display_mode: self.selected_display_mode(),
                    viewport_mode: self.selected_viewport_mode(),
                    quality: self.selected_quality(),
                });
                self.should_quit = true;
            }
            _ => {}
        }
    }

    pub(super) fn selected_display_mode(&self) -> DisplayMode {
        if self.render_index == 0 {
            DisplayMode::Rgb
        } else {
            DisplayMode::Ascii
        }
    }

    pub(super) fn selected_viewport_mode(&self) -> ViewportMode {
        if self.screen_index == 0 {
            ViewportMode::Fullscreen
        } else {
            ViewportMode::Cinema16x9
        }
    }

    pub(super) fn selected_quality(&self) -> RenderQuality {
        match self.quality_index {
            1 => RenderQuality::Balanced,
            2 => RenderQuality::Performance,
            _ => RenderQuality::Full,
        }
    }
}

pub(super) fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|file| file.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn scan_video_files() -> Result<Vec<PathBuf>> {
    let assets_dir = crate::utils::runtime::detect().assets_dir;
    let Some(video_dir) = constants::VIDEO_DIR_CANDIDATES
        .iter()
        .map(|candidate| assets_dir.join(candidate.strip_prefix("assets/").unwrap_or(candidate)))
        .find(|dir| dir.exists())
    else {
        return Ok(Vec::new());
    };

    let mut video_files: Vec<PathBuf> = fs::read_dir(&video_dir)
        .with_context(|| format!("failed to read {}", video_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| has_allowed_extension(path, constants::VIDEO_EXTENSIONS))
        .collect();

    video_files.sort();
    Ok(video_files)
}

fn scan_audio_files() -> Result<Vec<PathBuf>> {
    let assets_dir = crate::utils::runtime::detect().assets_dir;
    let audio_dir = assets_dir.join(
        constants::AUDIO_DIR
            .strip_prefix("assets/")
            .unwrap_or(constants::AUDIO_DIR),
    );
    if !audio_dir.exists() {
        return Ok(Vec::new());
    }

    let mut audio_files: Vec<PathBuf> = fs::read_dir(&audio_dir)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_step_is_between_render_and_screen() {
        let mut app = MenuApp {
            step: Step::Render,
            status: String::new(),
            should_quit: false,
            video_files: vec![PathBuf::from("video.mp4")],
            audio_files: Vec::new(),
            video_index: 0,
            audio_index: 0,
            render_index: 0,
            quality_index: 0,
            screen_index: 0,
            selection: None,
        };

        app.on_key(KeyCode::Enter);
        assert_eq!(app.step, Step::Quality);
        app.on_key(KeyCode::Enter);
        assert_eq!(app.step, Step::Screen);
    }

    #[test]
    fn selected_quality_is_preserved_in_selection() {
        let mut app = MenuApp {
            step: Step::Confirm,
            status: String::new(),
            should_quit: false,
            video_files: vec![PathBuf::from("video.mp4")],
            audio_files: Vec::new(),
            video_index: 0,
            audio_index: 0,
            render_index: 0,
            quality_index: 2,
            screen_index: 0,
            selection: None,
        };

        app.on_key(KeyCode::Enter);
        assert_eq!(app.selection.unwrap().quality, RenderQuality::Performance);
    }
}
