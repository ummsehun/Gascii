pub const APP_NAME: &str = "Gascii";

pub const GASCCI_CONFIG_FILE: &str = "Gascii.config";
pub const ERROR_LOG_FILE: &str = "error.log";
pub const DEBUG_LOG_FILE: &str = "debug.log";

pub const DEFAULT_FONT_SIZE: &str = "2.5";

pub const VIDEO_DIR_CANDIDATES: &[&str] = &["assets/video", "assets/vidio"];
pub const AUDIO_DIR: &str = "assets/audio";

pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "webm"];
pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "flac"];

pub const MENU_LOGO: &[&str] = &[
    "   _____                _ _ ",
    "  / ____|              (_|_)",
    " | |  __  __ _ ___  ___ _ _ ",
    " | | |_ |/ _` / __|/ __| | |",
    " | |__| | (_| \\__ \\ (__| | |",
    "  \\_____|\\__,_|___/\\___|_|_|",
];

pub const MENU_RENDER_MODE_LABELS: &[&str] =
    &["RGB TrueColor (최고 화질)", "ASCII 흑백 (텍스트 모드)"];
pub const MENU_SCREEN_MODE_LABELS: &[&str] = &["전체 화면 (꽉 차게)", "원본 비율 (16:9)"];
pub const MENU_NO_AUDIO_LABEL: &str = "오디오 없이 재생";

pub const WINDOWED_COLUMNS: u16 = 240;
pub const WINDOWED_ROWS: u16 = 68;
