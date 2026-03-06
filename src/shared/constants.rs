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
pub const MENU_SCREEN_MODE_LABELS: &[&str] = &["전체 화면 (원본비율 유지)", "16:9 비율 유지"];
pub const MENU_NO_AUDIO_LABEL: &str = "오디오 없이 재생";

pub const FRAME_QUEUE_CAPACITY: usize = 36;
pub const PERF_LOG_EVERY_FRAMES: u64 = 180;
pub const RGB_COLOR_DELTA_THRESHOLD: u8 = 2;
pub const RGB_COLOR_DELTA_THRESHOLD_MAX: u8 = 14;
pub const RGB_ADAPTIVE_THRESHOLD_QUEUE_START: usize = 6;
pub const RGB_ADAPTIVE_THRESHOLD_QUEUE_FULL: usize = 30;
pub const RGB_HIGH_DENSITY_CELL_COUNT: u32 = 22_000;
pub const RGB_HIGH_DENSITY_MAX_FPS: u32 = 24;
pub const AUDIO_SYNC_COMP_MS: u64 = 120;
pub const RGB_ALLOW_UPSCALE: bool = false;
pub const ASCII_ALLOW_UPSCALE: bool = false;
pub const MAX_UPSCALE_FACTOR: f64 = 1.0;

pub const ASCII_GRADIENT: &str =
    " .'`^\",:;Il!i><~+_-?][}{1)(|\\\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";
pub const ASCII_GAMMA: f32 = 1.10;

pub const GHOSTTY_BIN_NAME: &str = "ghostty";
pub const GHOSTTY_MACOS_APP_BIN: &str = "/Applications/Ghostty.app/Contents/MacOS/ghostty";
pub const GHOSTTY_MACOS_APP_BUNDLE: &str = "/Applications/Ghostty.app";
pub const GHOSTTY_FULLSCREEN_ARG: &str = "--fullscreen";
pub const GHOSTTY_WINDOW_INHERIT_FONT_SIZE_ARG: &str = "--window-inherit-font-size=false";

pub const PLAY_SUBCOMMAND: &str = "play-live";
