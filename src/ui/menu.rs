use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Select, console::Term};
use std::path::{Path, PathBuf};
use std::fs;

pub fn run_menu() -> Result<()> {
    // 0. Custom Font Size Selection (User Request)
    use dialoguer::Input;
    
    // Read current config to find default
    let config_path = Path::new("Gascii.config");
    let mut current_font_size = "2.5".to_string();
    if let Ok(content) = fs::read_to_string(config_path) {
        for line in content.lines() {
            if line.trim().starts_with("font-size") {
                if let Some(val) = line.split('=').nth(1) {
                    current_font_size = val.trim().to_string();
                }
                break;
            }
        }
    }

    let font_size_str: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("🔠 폰트 크기 입력 (예: 2.0, 4.0, 10.0)")
        .default(current_font_size)
        .interact_on(&Term::stderr())?;

    // Update Gascii.config immediately
    if let Ok(content) = fs::read_to_string(config_path) {
        let new_content = content.lines().map(|line| {
            if line.trim().starts_with("font-size") {
                format!("font-size = {}", font_size_str)
            } else {
                line.to_string()
            }
        }).collect::<Vec<String>>().join("\n");
        
        if let Err(e) = fs::write(config_path, new_content) {
            eprintln!("⚠️  Gascii.config 업데이트 실패: {}", e);
        } else {
            eprintln!("✅ Gascii.config 폰트 크기 저장 완료: {}", font_size_str);
        }
    }

    // 1. Scan for video files
    let video_dirs = vec![Path::new("assets/video"), Path::new("assets/vidio")];
    let mut video_dir = Path::new("assets/video");
    let mut found_dir = false;

    for dir in &video_dirs {
        if dir.exists() {
            video_dir = dir;
            found_dir = true;
            break;
        }
    }
    
    if !found_dir {
        eprintln!("❌ assets/video (또는 assets/vidio) 디렉토리를 찾을 수 없습니다.");
        return Ok(());
    }

    let audio_dir = Path::new("assets/audio");

    let mut video_files: Vec<PathBuf> = fs::read_dir(video_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            matches!(ext.as_str(), "mp4" | "mkv" | "avi" | "mov" | "webm")
        })
        .collect();

    video_files.sort();

    if video_files.is_empty() {
        eprintln!("❌ 재생할 비디오 파일이 없습니다.");
        return Ok(());
    }

    // 2. Select Video
    let video_names: Vec<String> = video_files.iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("📺 재생할 영상을 선택하세요")
        .default(0)
        .items(&video_names)
        .interact_on(&Term::stderr())?;

    let selected_video = &video_files[selection];

    // 3. Select Audio (Optional)
    let mut audio_files: Vec<PathBuf> = if audio_dir.exists() {
        fs::read_dir(audio_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                matches!(ext.as_str(), "wav" | "mp3" | "m4a" | "flac")
            })
            .collect()
    } else {
        vec![]
    };

    audio_files.sort();

    let mut audio_path = None;
    if !audio_files.is_empty() {
        // DEBUG: Print sorted file list
        eprintln!("\n🔍 DEBUG: 정렬된 오디오 파일 목록:");
        for (i, f) in audio_files.iter().enumerate() {
            eprintln!("  [{}] {}", i, f.file_name().unwrap().to_string_lossy());
        }
        
        let mut audio_names: Vec<String> = audio_files.iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        audio_names.insert(0, "오디오 없이 재생".to_string());

        let audio_selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("🎵 오디오 파일 선택")
            .default(0)
            .items(&audio_names)
            .interact_on(&Term::stderr())?;

        if audio_selection > 0 {
            let selected_file = &audio_files[audio_selection - 1];
            eprintln!("🔍 DEBUG: 선택된 인덱스: {}", audio_selection);
            eprintln!("🔍 DEBUG: audio_files[{}] = {}", audio_selection - 1, selected_file.display());
            audio_path = Some(selected_file.clone());
        }
    } else {
        eprintln!("⚠️ assets/audio 디렉토리에 오디오 파일이 없습니다.");
    }

    // 4. Select Mode
    let modes = vec![
        "RGB TrueColor (최고 화질)", 
        "ASCII 흑백 (텍스트 모드)"
    ];
    let mode_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("🎨 렌더링 모드 선택")
        .default(0)
        .items(&modes)
        .interact_on(&Term::stderr())?;

    let mode_str = if mode_selection == 0 { "rgb" } else { "ascii" };
    eprintln!("🔍 DEBUG: 선택된 렌더링 모드: {}", mode_str);

    // 5. Select Screen Mode
    let screen_modes = vec!["전체 화면 (꽉 차게)", "원본 비율 (16:9)"];
    let screen_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("🖥️ 화면 모드 선택")
        .default(0)
        .items(&screen_modes)
        .interact_on(&Term::stderr())?;

    let fill_str = if screen_selection == 0 { "true" } else { "false" };

    // Calculate Ghostty arguments
    let ghostty_args = if fill_str == "true" {
        "--fullscreen".to_string()
    } else {
        // For 16:9 aspect ratio with ~1:2 cell ratio, we need approx 3.55:1 col:row ratio
        // 240x68 provides a good large window
        "--window-width=240 --window-height=68".to_string()
    };

    // Output for shell script to parse
    // Use explicit write to ensure no buffering issues
    // We add a small delay to ensure previous output is flushed
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    use std::io::Write;
    let mut stdout = std::io::stdout();
    writeln!(stdout, "__BAD_APPLE_CONFIG__VIDEO_PATH={}", selected_video.to_string_lossy())?;
    if let Some(a) = audio_path {
        writeln!(stdout, "__BAD_APPLE_CONFIG__AUDIO_PATH={}", a.to_string_lossy())?;
    } else {
        writeln!(stdout, "__BAD_APPLE_CONFIG__AUDIO_PATH=")?;
    }
    writeln!(stdout, "__BAD_APPLE_CONFIG__RENDER_MODE={}", mode_str)?;
    writeln!(stdout, "__BAD_APPLE_CONFIG__FILL_SCREEN={}", fill_str)?;
    writeln!(stdout, "__BAD_APPLE_CONFIG__GHOSTTY_ARGS={}", ghostty_args)?;
    stdout.flush()?;

    Ok(())
}
