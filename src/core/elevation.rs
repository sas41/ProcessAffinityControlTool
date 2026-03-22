#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ElevationAttemptResult {
    Launched,
    Declined,
    Unavailable,
    Failed(String),
}

#[cfg(target_os = "windows")]
pub fn is_elevated() -> bool {
    windows_elevate::check_elevated().unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(target_os = "macos")]
pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn is_elevated() -> bool {
    false
}

#[allow(dead_code)]
pub fn try_relaunch_elevated(args: &[String]) -> ElevationAttemptResult {
    #[cfg(target_os = "windows")]
    {
        return try_relaunch_elevated_windows(args);
    }

    #[cfg(target_os = "linux")]
    {
        return try_relaunch_elevated_linux(args);
    }

    #[cfg(target_os = "macos")]
    {
        return try_relaunch_elevated_macos(args);
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = args;
        ElevationAttemptResult::Unavailable
    }
}

#[allow(dead_code)]
#[cfg(target_os = "windows")]
fn try_relaunch_elevated_windows(args: &[String]) -> ElevationAttemptResult {
    let _ = args;

    match windows_elevate::elevate() {
        Ok(_) => ElevationAttemptResult::Launched,
        Err(e) => {
            let msg = e.to_string();
            let lower = msg.to_lowercase();
            if lower.contains("canceled") || lower.contains("cancelled") {
                ElevationAttemptResult::Declined
            } else {
                ElevationAttemptResult::Failed(msg)
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn try_relaunch_elevated_linux(args: &[String]) -> ElevationAttemptResult {
    use std::process::Command;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return ElevationAttemptResult::Failed(e.to_string()),
    };

    let status = Command::new("pkexec").arg(exe).args(args).spawn();

    match status {
        Ok(_) => ElevationAttemptResult::Launched,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ElevationAttemptResult::Unavailable,
        Err(e) => ElevationAttemptResult::Failed(e.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn try_relaunch_elevated_macos(args: &[String]) -> ElevationAttemptResult {
    use std::process::Command;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return ElevationAttemptResult::Failed(e.to_string()),
    };

    let mut cmdline = shell_quote(exe.to_string_lossy().as_ref());
    for arg in args {
        cmdline.push(' ');
        cmdline.push_str(&shell_quote(arg));
    }

    let script = format!(
        "do shell script {} with administrator privileges",
        shell_quote(&cmdline)
    );

    let output = Command::new("osascript").arg("-e").arg(script).output();

    match output {
        Ok(out) if out.status.success() => ElevationAttemptResult::Launched,
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
            if stderr.contains("user canceled") {
                ElevationAttemptResult::Declined
            } else {
                ElevationAttemptResult::Failed(
                    String::from_utf8_lossy(&out.stderr).trim().to_string(),
                )
            }
        }
        Err(e) => ElevationAttemptResult::Failed(e.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
