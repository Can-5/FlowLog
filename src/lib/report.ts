// FlowLog — pure report logic: aggregation, categorization, exports.

export interface RawEvent {
  ts: number
  app: string
  title: string
  process: string
}

export interface AppUsage {
  app: string
  process: string
  seconds: number
}

export interface CategoryUsage {
  category: string
  seconds: number
}

/** Each event covers the gap until the next event, capped at `cap` seconds. */
export function aggregate(events: RawEvent[], nowSec: number, capSec = 300): AppUsage[] {
  const sorted = [...events].sort((a, b) => a.ts - b.ts)
  const totals = new Map<string, AppUsage>()
  for (let i = 0; i < sorted.length; i++) {
    const ev = sorted[i]
    const nextTs = i + 1 < sorted.length ? sorted[i + 1].ts : nowSec
    const dur = Math.min(Math.max(0, nextTs - ev.ts), capSec)
    const key = ev.process || ev.app
    const cur = totals.get(key) ?? { app: ev.app, process: ev.process, seconds: 0 }
    cur.seconds += dur
    totals.set(key, cur)
  }
  return [...totals.values()].sort((a, b) => b.seconds - a.seconds)
}

/** 24 day-buckets (seconds) for a timeline strip. */
export function timeline(events: RawEvent[], dayStartSec: number, nowSec: number, capSec = 300): number[] {
  const buckets = new Array(24).fill(0)
  const sorted = [...events].sort((a, b) => a.ts - b.ts)
  for (let i = 0; i < sorted.length; i++) {
    const ev = sorted[i]
    const nextTs = i + 1 < sorted.length ? sorted[i + 1].ts : nowSec
    let remaining = Math.min(Math.max(0, nextTs - ev.ts), capSec)
    let t = ev.ts
    while (remaining > 0) {
      const daySec = t - dayStartSec
      if (daySec >= 0 && daySec < 24 * 3600) {
        const hour = Math.floor(daySec / 3600)
        const inHour = Math.min(remaining, 3600 - (daySec % 3600))
        buckets[hour] += inHour
        remaining -= inHour
        t += inHour
      } else if (daySec < 0) {
        t = dayStartSec + remaining // skip events before this day
        remaining = 0
      } else {
        remaining = 0
      }
    }
  }
  return buckets
}

export const DEFAULT_CATEGORY_RULES: [string, string][] = [
  ['Development', ['code', 'cursor', 'idea', 'webstorm', 'vim', 'nvim', 'emacs', 'terminal', 'konsole', 'kitty', 'wezterm', 'alacritty', 'git', 'docker', 'cargo', 'rustc', 'node', 'python', 'java', 'gcc']],
  ['Browser', ['firefox', 'chrome', 'chromium', 'brave', 'edge', 'vivaldi', 'opera', 'librewolf']],
  ['Communication', ['slack', 'telegram', 'discord', 'teams', 'zoom', 'kmail', 'thunderbird', 'signal', 'element']],
  ['Office', ['writer', 'calc', 'impress', 'onlyoffice', 'word', 'excel', 'powerpoint', 'libreoffice', 'okular', 'evince', 'acrobat']],
  ['Media', ['spotify', 'vlc', 'mpv', 'audacity', 'kdenlive', 'obs', 'celluloid', 'strawberry', 'rhythmbox']],
  ['Gaming', ['steam', 'lutris', 'heroic', 'gamemode']],
  ['System', ['dolphin', 'nautilus', 'thunar', 'files', 'systemmonitor', 'gnome-system']],
].flatMap(([cat, kws]) => (kws as string[]).map((kw) => [kw, cat] as [string, string]))

export const CATEGORIES = ['Development', 'Browser', 'Communication', 'Office', 'Media', 'Gaming', 'System', 'Other']

export function categorize(process: string, userRules: [string, string][] = []): string {
  const p = process.toLowerCase()
  for (const [kw, cat] of userRules) {
    if (p.includes(kw.toLowerCase())) return cat
  }
  for (const [kw, cat] of DEFAULT_CATEGORY_RULES) {
    if (p.includes(kw)) return cat
  }
  return 'Other'
}

export function byCategory(
  usage: AppUsage[],
  userRules: [string, string][] = []
): CategoryUsage[] {
  const totals = new Map<string, number>()
  for (const u of usage) {
    const cat = categorize(u.process, userRules)
    totals.set(cat, (totals.get(cat) ?? 0) + u.seconds)
  }
  return [...totals.entries()]
    .map(([category, seconds]) => ({ category, seconds }))
    .sort((a, b) => b.seconds - a.seconds)
}

/** Local start-of-day / start-of-week (Monday) / start-of-month as unix seconds. */
export function rangeStart(kind: 'today' | 'week' | 'month', now = new Date()): number {
  const d = new Date(now)
  d.setHours(0, 0, 0, 0)
  if (kind === 'week') {
    const weekday = (d.getDay() + 6) % 7 // Monday = 0
    d.setDate(d.getDate() - weekday)
  } else if (kind === 'month') {
    d.setDate(1)
  }
  return Math.floor(d.getTime() / 1000)
}

export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.round(seconds))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  const sec = s % 60
  return m > 0 ? `${m}m ${sec}s` : `${sec}s`
}

function csvEscape(v: string): string {
  if (/[",\n]/.test(v)) return `"${v.replace(/"/g, '""')}"`
  return v
}

/** CSV of raw event durations (seconds until next event, capped). */
export function buildCsv(events: RawEvent[], nowSec: number): string {
  const sorted = [...events].sort((a, b) => a.ts - b.ts)
  const rows = ['timestamp_iso,app,title,process,seconds']
  for (let i = 0; i < sorted.length; i++) {
    const ev = sorted[i]
    const nextTs = i + 1 < sorted.length ? sorted[i + 1].ts : nowSec
    const dur = Math.min(Math.max(0, nextTs - ev.ts), 300)
    const iso = new Date(ev.ts * 1000).toISOString()
    rows.push([iso, ev.app, ev.title, ev.process, String(dur)].map(csvEscape).join(','))
  }
  return rows.join('\n') + '\n'
}

export function buildJsonReport(
  events: RawEvent[],
  rangeKind: string,
  fromSec: number,
  nowSec: number,
  userRules: [string, string][] = [],
): string {
  const usage = aggregate(events, nowSec)
  const cats = byCategory(usage, userRules)
  return JSON.stringify(
    {
      app: 'FlowLog',
      exported_at: new Date().toISOString(),
      range: rangeKind,
      from: new Date(fromSec * 1000).toISOString(),
      to: new Date(nowSec * 1000).toISOString(),
      total_seconds: usage.reduce((a, b) => a + b.seconds, 0),
      by_app: usage,
      by_category: cats,
      events: events.length,
    },
    null,
    2
  )
}
