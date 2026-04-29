pub fn profiling_enabled() -> bool {
    std::env::var("GASCII_PROFILE_MEMORY")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

pub fn max_rss_bytes() -> Option<u64> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
        if result != 0 {
            return None;
        }

        let max_rss = unsafe { usage.assume_init().ru_maxrss as u64 };
        #[cfg(target_os = "macos")]
        {
            Some(max_rss)
        }
        #[cfg(target_os = "linux")]
        {
            Some(max_rss.saturating_mul(1024))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    format!("{:.1}MiB", bytes as f64 / MIB)
}
