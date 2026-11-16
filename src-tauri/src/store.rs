// Local JSONL event log + small state file. No data ever leaves the machine.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::tracker::{active_window, ActiveWindow};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ts: u64,
    pub app: String,
    pub title: String,
    pub process: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerStateFile {
    pub paused: bool,
    pub excluded: Vec<String>,
}
impl Default for TrackerStateFile {
    fn default() -> Self {
        Self { paused: false, excluded: vec!["lockapp".into()] }
    }
}

pub struct Tracker {
    state_path: PathBuf,
    log_path: PathBuf,
    state: TrackerStateFile,
    last_key: Option<(u64, String, String)>, // (ts, app, title)
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TickResult {
    pub recorded: bool,
    pub paused: bool,
    pub supported: bool,
    pub reason: Option<String>,
    pub current: Option<ActiveWindow>,
}

impl Tracker {
    pub fn load(data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&data_dir);
        let state_path = data_dir.join("flowlog.json");
        let log_path = data_dir.join("events.jsonl");
        let state: TrackerStateFile = fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let last_key = last_event_key(&log_path);
        Self { state_path, log_path, state, last_key, last_error: None }
    }

    fn save_state(&self) -> Result<(), String> {
        let tmp = self.state_path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(&self.state).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.state_path).map_err(|e| e.to_string())
    }

    pub fn paused(&self) -> bool { self.state.paused }
    pub fn excluded(&self) -> Vec<String> { self.state.excluded.clone() }

    pub fn set_paused(&mut self, paused: bool) -> Result<(), String> {
        self.state.paused = paused;
        self.save_state()
    }

    pub fn set_excluded(&mut self, apps: Vec<String>) -> Result<(), String> {
        self.state.excluded = apps
            .into_iter()
            .map(|a| a.trim().to_lowercase())
            .filter(|a| !a.is_empty())
            .collect();
        self.save_state()
    }

    fn is_excluded(&self, process: &str) -> bool {
        let p = process.to_lowercase();
        self.state
            .excluded
            .iter()
            .any(|ex| p == *ex || p.contains(ex.as_str()))
    }

    /// One sampling tick. Records an event only on (app,title) change or every 45 s.
    pub fn tick(&mut self) -> TickResult {
        let paused = self.state.paused;
        if paused {
            return TickResult { recorded: false, paused, supported: true, reason: Some("paused".into()), current: None };
        }
        match active_window() {
            Err(reason) => TickResult { recorded: false, paused, supported: false, reason: Some(reason), current: None },
            Ok(None) => TickResult { recorded: false, paused, supported: true, reason: None, current: None },
            Ok(Some(win)) => {
                if self.is_excluded(&win.process) {
                    return TickResult { recorded: false, paused, supported: true, reason: Some("excluded app".into()), current: Some(win) };
                }
                let now = unix_now();
                let key = (now, win.app.clone(), win.title.clone());
                let should_write = match &self.last_key {
                    Some((ts, app, title)) => app != &win.app || title != &win.title || now.saturating_sub(*ts) >= 45,
                    None => true,
                };
                if should_write {
                    if let Err(e) = self.append_event(now, &win) {
                        self.last_error = Some(e.clone());
                        return TickResult { recorded: false, paused, supported: true, reason: Some(e), current: Some(win) };
                    }
                    self.last_key = Some(key);
                }
                TickResult { recorded: should_write, paused, supported: true, reason: None, current: Some(win) }
            }
        }
    }

    fn append_event(&self, ts: u64, win: &ActiveWindow) -> Result<(), String> {
        let mut f = OpenOptions::new().create(true).append(true).open(&self.log_path).map_err(|e| e.to_string())?;
        let ev = serde_json::json!({ "ts": ts, "app": win.app, "title": win.title, "process": win.process });
        writeln!(f, "{ev}").map_err(|e| e.to_string())
    }

    pub fn events_between(&self, start: u64, end: u64) -> Vec<Event> {
        let file = match fs::File::open(&self.log_path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let line = line.trim().to_string();
            if line.is_empty() { continue; }
            if let Ok(ev) = serde_json::from_str::<Event>(&line) {
                if ev.ts >= start && ev.ts <= end {
                    out.push(ev);
                }
            }
        }
        out.sort_by_key(|e| e.ts);
        out
    }

    pub fn clear_events(&self) -> Result<(), String> {
        if self.log_path.exists() {
            fs::remove_file(&self.log_path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn event_count(&self) -> u64 {
        fs::File::open(&self.log_path)
            .map(|f| BufReader::new(f).lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).count() as u64)
            .unwrap_or(0)
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn last_event_key(log_path: &PathBuf) -> Option<(u64, String, String)> {
    let file = fs::File::open(log_path).ok()?;
    let mut last = None;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Ok(ev) = serde_json::from_str::<Event>(&line) {
            last = Some((ev.ts, ev.app, ev.title));
        }
    }
    last
}

/// Validates a user-chosen export filename; returns the full path inside `download_dir`.
pub fn safe_export_path(download_dir: &PathBuf, filename: &str) -> Result<PathBuf, String> {
    let name = filename.trim();
    if name.is_empty() {
        return Err("filename is empty".into());
    }
    if name.contains(['/', '\\', '\0']) {
        return Err("filename must not contain path separators".into());
    }
    if name.starts_with('.') || name.len() > 120 {
        return Err("invalid filename".into());
    }
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !matches!(ext.as_str(), "csv" | "json" | "md" | "txt") {
        return Err("allowed export extensions: csv, json, md, txt".into());
    }
    Ok(download_dir.join(name))
}

pub struct Shared(pub Mutex<Tracker>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_path_validation() {
        let dl = PathBuf::from("/tmp/downloads");
        assert!(safe_export_path(&dl, "report.csv").is_ok());
        assert!(safe_export_path(&dl, "../evil.csv").is_err());
        assert!(safe_export_path(&dl, "a/b.csv").is_err());
        assert!(safe_export_path(&dl, "report.exe").is_err());
        assert!(safe_export_path(&dl, ".hidden").is_err());
        assert!(safe_export_path(&dl, "").is_err());
    }

    #[test]
    fn events_roundtrip_and_filter() {
        let dir = std::env::temp_dir().join(format!("flowlog-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let tracker = Tracker::load(dir.clone());
        // corrupt line must be skipped
        fs::write(dir.join("events.jsonl"), "not json\n{\"ts\":100,\"app\":\"a\",\"title\":\"t\",\"process\":\"p\"}\n{\"ts\":5000,\"app\":\"b\",\"title\":\"u\",\"process\":\"q\"}\n").unwrap();
        let evs = tracker.events_between(0, 10_000);
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].app, "a");
        let only = tracker.events_between(200, 10_000);
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].app, "b");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn excluded_matching_is_case_insensitive_substring() {
        let dir = std::env::temp_dir().join(format!("flowlog-excl-test-{}", std::process::id()));
        let mut tracker = Tracker::load(dir.clone());
        tracker.set_excluded(vec!["FIREfox".into(), "  ".into()]).unwrap();
        assert!(tracker.is_excluded("firefox"));
        assert!(tracker.is_excluded("Firefox"));
        assert!(!tracker.is_excluded("code"));
        let _ = fs::remove_dir_all(&dir);
    }
}
