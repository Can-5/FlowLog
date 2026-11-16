// Active-window / frontmost-app detection, per platform.
// Returns Err(reason) on unsupported sessions (Wayland) — never fakes data.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ActiveWindow {
    pub app: String,
    pub title: String,
    pub process: String,
}

#[cfg(target_os = "linux")]
pub fn active_window() -> Result<Option<ActiveWindow>, String> {
    if std::env::var("XDG_SESSION_TYPE").map(|v| v == "wayland").unwrap_or(false) {
        return Err(
            "Wayland session detected. Active-window detection needs X11 (or a portal the app doesn't use yet). \
             FlowLog is recording nothing rather than pretending."
                .into(),
        );
    }
    // X11 path: _NET_ACTIVE_WINDOW + WM_CLASS + _NET_WM_NAME via xprop
    let root = std::process::Command::new("xprop")
        .args(["-root", "_NET_ACTIVE_WINDOW"])
        .output()
        .map_err(|e| format!("xprop not available: {e}. Install xprop for X11 tracking."))?;
    let root_str = String::from_utf8_lossy(&root.stdout);
    let win_id = root_str
        .split("window id # ")
        .nth(1)
        .and_then(|s| s.trim().split_whitespace().next())
        .ok_or("could not parse _NET_ACTIVE_WINDOW")?;
    if win_id == "0x0" || win_id.is_empty() {
        return Ok(None); // no focused window (e.g. desktop)
    }
    let detail = std::process::Command::new("xprop")
        .args(["-id", win_id, "WM_CLASS", "_NET_WM_NAME"])
        .output()
        .map_err(|e| format!("xprop failed on window {win_id}: {e}"))?;
    let out = String::from_utf8_lossy(&detail.stdout);
    let mut process = String::new();
    let mut title = String::new();
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("WM_CLASS(STRING)") {
            // format: WM_CLASS(STRING) = "instance", "Class"
            if let Some(tail) = rest.split('=').nth(1) {
                process = tail
                    .split(',')
                    .nth(1)
                    .unwrap_or(tail)
                    .trim()
                    .trim_matches('"')
                    .to_string();
            }
        } else if let Some(rest) = line.strip_prefix("_NET_WM_NAME(UTF8_STRING)") {
            if let Some(tail) = rest.split('=').nth(1) {
                title = tail.trim().trim_matches('"').to_string();
            }
        }
    }
    if process.is_empty() && title.is_empty() {
        return Ok(None);
    }
    if process.is_empty() {
        process = title.clone();
    }
    Ok(Some(ActiveWindow { app: process.clone(), title: title.clone(), process }))
}

#[cfg(target_os = "macos")]
pub fn active_window() -> Result<Option<ActiveWindow>, String> {
    let script = r#"
tell application "System Events"
    set frontApp to name of first application process whose frontmost is true
end tell
set windowTitle to ""
tell application frontApp
    try
        set windowTitle to name of front window
    end try
end tell
return frontApp & "\\n" & windowTitle
"#;
    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("osascript failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "osascript returned an error. Grant Accessibility permission to FlowLog: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let app = lines.next().unwrap_or("").trim().to_string();
    let title = lines.next().unwrap_or("").trim().to_string();
    if app.is_empty() {
        return Ok(None);
    }
    Ok(Some(ActiveWindow { app: app.clone(), title, process: app }))
}

#[cfg(target_os = "windows")]
pub fn active_window() -> Result<Option<ActiveWindow>, String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Ok(None);
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        let title = String::from_utf16_lossy(&buf[..len as usize]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut process = String::new();
        if pid != 0 {
            if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut path = [0u16; 1024];
                let mut size = path.len() as u32;
                if QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR::from_raw(path.as_mut_ptr()), &mut size).is_ok() {
                    let full = String::from_utf16_lossy(&path[..size as usize]);
                    process = full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string();
                }
                let _ = CloseHandle(handle);
            }
        }
        if process.is_empty() && title.is_empty() {
            return Ok(None);
        }
        Ok(Some(ActiveWindow {
            app: process.clone(),
            title,
            process,
        }))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn active_window() -> Result<Option<ActiveWindow>, String> {
    Err("This platform is not supported yet.".into())
}
