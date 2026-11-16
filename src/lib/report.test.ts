import { describe, it, expect } from 'vitest'
import { aggregate, byCategory, categorize, rangeStart, formatDuration, buildCsv, timeline, type RawEvent } from './report'

const ev = (ts: number, app: string, process = app): RawEvent => ({ ts, app, title: `${app} window`, process })

describe('aggregate', () => {
  it('sums durations until next event, caps gaps', () => {
    const events = [ev(100, 'A'), ev(160, 'B'), ev(200, 'A')]
    const usage = aggregate(events, 260, 300)
    expect(usage.find((u) => u.app === 'A')?.seconds).toBe(60 + 60)
    expect(usage.find((u) => u.app === 'B')?.seconds).toBe(40)
  })
  it('caps huge idle gaps at 300s', () => {
    const usage = aggregate([ev(100, 'A')], 10_000, 300)
    expect(usage[0].seconds).toBe(300)
  })
  it('sorts by usage descending', () => {
    const usage = aggregate([ev(0, 'A'), ev(10, 'B')], 1000, 300)
    expect(usage[0].app).toBe('B')
  })
})

describe('categorize', () => {
  it('matches built-in keywords case-insensitively', () => {
    expect(categorize('Firefox')).toBe('Browser')
    expect(categorize('code')).toBe('Development')
    expect(categorize('xyzzy')).toBe('Other')
  })
  it('user rules win over built-ins', () => {
    expect(categorize('firefox', [['firefox', 'Work']])).toBe('Work')
  })
})

describe('byCategory', () => {
  it('groups usage by category', () => {
    const usage = [
      { app: 'Firefox', process: 'firefox', seconds: 100 },
      { app: 'Code', process: 'code', seconds: 300 },
    ]
    const cats = byCategory(usage)
    expect(cats.find((c) => c.category === 'Development')?.seconds).toBe(300)
    expect(cats.find((c) => c.category === 'Browser')?.seconds).toBe(100)
  })
})

describe('rangeStart', () => {
  it('today/week/month return midnight-aligned distinct starts', () => {
    const now = new Date(2026, 10, 17, 15, 30)
    const today = rangeStart('today', now)
    const week = rangeStart('week', now)
    const month = rangeStart('month', now)
    expect(new Date(today * 1000).getHours()).toBe(0)
    expect(week).toBeLessThanOrEqual(today)
    expect(month).toBeLessThanOrEqual(week)
  })
})

describe('formatDuration', () => {
  it('formats hours/minutes/seconds', () => {
    expect(formatDuration(3661)).toBe('1h 1m')
    expect(formatDuration(95)).toBe('1m 35s')
    expect(formatDuration(42)).toBe('42s')
  })
})

describe('timeline', () => {
  it('distributes an event across hour buckets', () => {
    const dayStart = new Date(2026, 10, 17, 0, 0, 0).getTime() / 1000
    const buckets = timeline([ev(dayStart + 3500, 'A')], dayStart, dayStart + 7200, 300)
    expect(buckets[0]).toBe(100) // 3500→3600
    expect(buckets[1]).toBe(200) // 3600→3800
    expect(buckets.reduce((a, b) => a + b, 0)).toBe(300)
  })
})

describe('buildCsv', () => {
  it('escapes quotes/commas and uses iso timestamps', () => {
    const csv = buildCsv([{ ts: 1_700_000_000, app: 'A, b', title: 'she said "hi"', process: 'p' }], 1_700_000_100)
    const lines = csv.trim().split('\n')
    expect(lines[0]).toBe('timestamp_iso,app,title,process,seconds')
    expect(lines[1]).toContain('"A, b"')
    expect(lines[1]).toContain('"she said ""hi"""')
  })
})
