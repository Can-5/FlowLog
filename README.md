# FlowLog ⏱️

A private, local computer-activity log for Linux, Windows and macOS. FlowLog notices which app is in the foreground, records app-switch events to a plain JSONL file on **your** device, and turns them into usage statistics and exportable reports. No screenshots. No keystrokes. No account. No telemetry. Nothing leaves the machine.

## Features

- **Frontmost-app tracking** — samples the active app every 5 s; records only when the app changes (plus a 45 s heartbeat).
- **Today dashboard** — tracked time, most-used app, app switches, current app, top-8 app bar chart, 24-hour timeline, category breakdown.
- **Reports** — 7-day / 30-day / this-month views, per-app table with share, category totals, **CSV and JSON export** (written to your Downloads folder).
- **Pause / resume** — stop tracking at any time; the header pill shows recording state.
- **Exclusions** — one process keyword per line (e.g. `firefox`, `steam`); matching apps are never recorded.
- **Custom category rules** — map process keywords to Development / Browser / Communication / Office / Media / Gaming / System / Other.
- **Erase everything** — one click (two-step) wipes all recorded data.

## Requirements

Same as the other apps: Node 22+, pnpm 10+, Rust 1.85+, and platform webview dependencies (see [Platform notes](#platform-notes)).

## Install & run

```bash
pnpm install
pnpm tauri dev        # first run builds the Rust backend
pnpm tauri build       # production build (desktop installer/bundle)
```

## How tracking works

| Platform | Mechanism | Notes |
|----------|-----------|-------|
| Linux (X11) | `xprop -root _NET_ACTIVE_WINDOW` + `xprop -id … WM_CLASS _NET_WM_NAME` | install `xprop` if missing |
| Linux (Wayland) | ❌ not available | FlowLog shows an honest "unavailable" banner instead of fake data |
| macOS | `osascript` – System Events frontmost app + front window name | grant Accessibility permission when prompted |
| Windows | `GetForegroundWindow` + `QueryFullProcessImageNameW` (user32/kernel32 via `windows` crate) | no extra installs |

Events are stored as newline-delimited JSON in the app data directory
(`~/.local/share/com.can5.flowlog/events.jsonl` on Linux,
`%APPDATA%\com.can5.flowlog\events.jsonl` on Windows,
`~/Library/Application Support/com.can5.flowlog/events.jsonl` on macOS).
State (pause flag, exclusions) lives in `flowlog.json` next to it.

Durations in reports are derived from consecutive event timestamps, capped at 300 s to discount idle gaps.

## Privacy

- Everything is stored locally; no network access is used by the tracking path.
- No screenshots, keylogging, or window-content capture — only app name, title and process name.
- Exports go only to your Downloads folder, with a filename whitelist (`csv`, `json`, `md`, `txt`).
- Delete all data from **Settings → Erase all tracked data**, or delete the app-data folder after uninstalling.

## Platform notes & limitations

- **Wayland**: active-window detection is not available; the app still opens and shows an explicit banner, and never fabricates data.
- **macOS**: without Accessibility permission, `osascript` fails and FlowLog reports it instead of guessing.
- **Linux**: window class is taken from `WM_CLASS` (the X11 class, e.g. `firefox`).
- Wayland support would require the `ext-foreign-toplevel` / `zwlr_foreign_toplevel_manager_v1` protocol — planned; not in this release.

## Development

```bash
pnpm dev           # vite dev server
pnpm test          # unit tests (aggregation, export, categories)
cargo test         # Rust unit tests (store, export-path safety)
pnpm build         # frontend build
pnpm tauri build  # full desktop build
```
