use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::panic;
use std::backtrace::Backtrace;
use lazy_static::lazy_static;

lazy_static! {
    static ref LOGGER: Mutex<Option<String>> = Mutex::new(None);
}

pub fn init(log_file: &str) {
    let mut log_path = std::env::current_dir().unwrap_or_default();
    log_path.push(log_file);
    
    // Initialize/Truncate file
    if let Ok(mut file) = OpenOptions::new().create(true).write(true).truncate(true).open(&log_path) {
        let _ = writeln!(file, "=== Log Started: {} ===", chrono::Local::now());
    }

    let path_str = log_path.to_string_lossy().to_string();
    *LOGGER.lock().unwrap() = Some(path_str.clone());

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
        
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "unknown".to_string());
        
        let error_msg = format!(
            "\nCRITICAL PANIC at {}:\nMessage: {}\nBacktrace:\n{:?}\n",
            location, msg, backtrace
        );
        
        // Write to file immediately
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path_str) {
            let _ = writeln!(file, "{}", error_msg);
        }
        
        // Also try to restore terminal if possible (best effort)
        let _ = crossterm::terminal::disable_raw_mode();
        println!("Application crashed. See {} for details.", path_str);
    }));
}

pub fn log(level: &str, msg: &str) {
    if let Some(path) = LOGGER.lock().unwrap().as_ref() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            let _ = writeln!(file, "[{}][{}] {}", timestamp, level, msg);
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
