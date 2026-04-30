use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Development,
    Production,
}

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub mode: RuntimeMode,
    pub exe_path: Option<PathBuf>,
    pub package_root: Option<PathBuf>,
    pub assets_dir: PathBuf,
    pub log_dir: PathBuf,
}

pub fn detect() -> RuntimePaths {
    let exe_path = std::env::current_exe().ok();
    let package_root = exe_path.as_ref().and_then(detect_package_root);
    let mode = if package_root.is_some() {
        RuntimeMode::Production
    } else {
        RuntimeMode::Development
    };

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let assets_dir = std::env::var_os("GASCII_ASSETS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| match &package_root {
            Some(root) => root.join("assets"),
            None => current_dir.join("assets"),
        });
    let log_dir = std::env::var_os("GASCII_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| match &package_root {
            Some(root) => root.join("logs"),
            None => current_dir,
        });

    RuntimePaths {
        mode,
        exe_path,
        package_root,
        assets_dir,
        log_dir,
    }
}

fn detect_package_root(exe_path: &PathBuf) -> Option<PathBuf> {
    let exe_dir = exe_path.parent()?;
    let candidates = if exe_dir.file_name().and_then(|name| name.to_str()) == Some("bin") {
        vec![exe_dir.parent()?.to_path_buf(), exe_dir.to_path_buf()]
    } else {
        vec![exe_dir.to_path_buf()]
    };

    candidates
        .into_iter()
        .find(|root| root.join("manifest.json").exists() || root.join("assets").exists())
}
