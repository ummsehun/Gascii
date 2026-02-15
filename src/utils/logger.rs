use crate::shared::constants;
use lazy_static::lazy_static;
use std::backtrace::Backtrace;
use std::fs::OpenOptions;
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone)]
struct LoggerPaths {
    error_path: String,
    debug_path: String,
}

lazy_static! {
    static ref LOGGER: Mutex<Option<LoggerPaths>> = Mutex::new(None);
}

fn append_line(path: &str, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", line);
    }
}

pub fn init() {
    let mut error_path = std::env::current_dir().unwrap_or_default();
    error_path.push(constants::ERROR_LOG_FILE);

    let mut debug_path = PathBuf::from(&error_path);
    debug_path.set_file_name(constants::DEBUG_LOG_FILE);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&error_path)
    {
        let _ = writeln!(file, "=== Error Log Started: {} ===", chrono::Local::now());
    }

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&debug_path)
    {
        let _ = writeln!(file, "=== Debug Log Started: {} ===", chrono::Local::now());
    }

    let paths = LoggerPaths {
        error_path: error_path.to_string_lossy().to_string(),
        debug_path: debug_path.to_string_lossy().to_string(),
    };
    *LOGGER.lock().unwrap() = Some(paths.clone());

    // Set panic hook
    panic::set_hook(Box::new(move |info| {
        let backtrace = Backtrace::capture();
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());

        let error_msg = format!(
            "\nCRITICAL PANIC at {}:\nMessage: {}\nBacktrace:\n{:?}\n",
            location, msg, backtrace
        );

        append_line(&paths.error_path, &error_msg);
        append_line(&paths.debug_path, &error_msg);

        // Also try to restore terminal if possible (best effort)
        let _ = crossterm::terminal::disable_raw_mode();
        println!("Application crashed. See {} for details.", paths.error_path);
    }));
}

pub fn log(level: &str, msg: &str) {
    if let Some(paths) = LOGGER.lock().unwrap().as_ref() {
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
        let line = format!("[{}][{}] {}", timestamp, level, msg);
        append_line(&paths.debug_path, &line);

        if level == "ERROR" {
            append_line(&paths.error_path, &line);
        }
    }
}

pub fn info(msg: &str) {
    log("INFO", msg);
}

pub fn error(msg: &str) {
    log("ERROR", msg);
}

pub fn debug(msg: &str) {
    log("DEBUG", msg);
}
