<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import {
    aggregate, byCategory, categorize, rangeStart, formatDuration, buildCsv, buildJsonReport, timeline,
    CATEGORIES, type RawEvent,
  } from '$lib/report'
  import { cycleTheme } from '$lib/theme'

  type Tab = 'today' | 'reports' | 'settings'
  type ReportRange = '7d' | '30d' | 'month'

  const nowSec = () => Math.floor(Date.now() / 1000)

  let tab = $state<Tab>('today')
  let unsupportedReason = $state<string | null>(null)
  let paused = $state(false)
  let currentApp = $state<string | null>(null)
  let trackedEvents = $state(0)
  let todayEvents = $state<RawEvent[]>([])
  let reportEvents = $state<RawEvent[]>([])
  let reportRange = $state<ReportRange>('7d')
  let userRules = $state<[string, string][]>(load<[string, string][]>('flowlog:rules', []))
  let ruleKw = $state('')
  let ruleCat = $state<string>('Development')
  let excludedText = $state('')
  let eraseArmed = $state(false)
  let toast = $state('')
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let poll: ReturnType<typeof setInterval> | undefined

  function load<T>(key: string, fallback: T): T {
    try {
      const raw = localStorage.getItem(key)
      return raw ? (JSON.parse(raw) as T) : fallback
    } catch { return fallback }
  }

  function showToast(msg: string) {
    toast = msg
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast = ''), 4200)
  }

  const todayUsage = $derived(aggregate(todayEvents, nowSec()))
  const todayTotal = $derived(todayUsage.reduce((a, b) => a + b.seconds, 0))
  const todayCats = $derived(byCategory(todayUsage, userRules))
  const todaySwitches = $derived(todayEvents.length)
  const todayMost = $derived(todayUsage[0]?.app ?? null)
  const dayStart = $derived(rangeStart('today'))
  const timelineBuckets = $derived(timeline(todayEvents, dayStart, nowSec()))
  const maxBucket = $derived(Math.max(...timelineBuckets, 1))

  const reportStart = $derived(
    reportRange === 'month' ? rangeStart('month') : nowSec() - Number(reportRange) * 86400
  )
  const reportUsage = $derived(aggregate(reportEvents, nowSec()))
  const reportTotal = $derived(reportUsage.reduce((a, b) => a + b.seconds, 0))
  const reportCats = $derived(byCategory(reportUsage, userRules))
  const maxReportSec = $derived(Math.max(...reportUsage.map((u) => u.seconds), 1))

  async function refresh() {
    try {
      const t = await invoke<{ supported: boolean; reason: string | null; paused: boolean; current: { app: string } | null }>('tick')
      paused = t.paused
      unsupportedReason = t.supported ? null : (t.reason ?? 'Active-window detection is unavailable.')
      currentApp = t.current?.app ?? null
      todayEvents = await invoke<RawEvent[]>('events_between', { start: rangeStart('today'), end: nowSec() })
      reportEvents = await invoke<RawEvent[]>('events_between', { start: reportStart, end: nowSec() })
      const status = await invoke<{ tracked_events: number }>('tracker_status')
      trackedEvents = status.tracked_events
    } catch (err) {
      console.error('FlowLog refresh failed', err)
    }
  }

  async function togglePaused() {
    await invoke('set_paused', { paused: !paused })
    await refresh()
    showToast(paused ? 'Tracking paused — nothing new is recorded' : 'Tracking resumed')
  }

  async function saveExcluded() {
    const apps = excludedText.split('\n').map((s) => s.trim()).filter(Boolean)
    await invoke('set_excluded', { apps })
    showToast('Exclusion list saved')
  }

  function addRule() {
    const kw = ruleKw.trim().toLowerCase()
    if (!kw) return
    userRules = [[kw, ruleCat], ...userRules]
    persistRules()
    ruleKw = ''
  }

  function removeRule(i: number) {
    userRules = userRules.filter((_, idx) => idx !== i)
    persistRules()
  }

  function persistRules() {
    try { localStorage.setItem('flowlog:rules', JSON.stringify(userRules)) } catch { /* quota */ }
  }

  async function exportReport(kind: 'csv' | 'json') {
    const stamp = new Date().toISOString().slice(0, 10)
    const filename = `flowlog-${reportRange}-${stamp}.${kind}`
    const content = kind === 'csv'
      ? buildCsv(reportEvents, nowSec())
      : buildJsonReport(reportEvents, reportRange, reportStart, nowSec(), userRules)
    try {
      const path = await invoke<string>('write_export', { filename, content })
      showToast(`Saved to ${path}`)
    } catch (err) {
      showToast(String(err))
    }
  }

  async function eraseAll() {
    if (!eraseArmed) {
      eraseArmed = true
      return
    }
    await invoke('clear_events')
    eraseArmed = false
    todayEvents = []
    reportEvents = []
    showToast('All tracked data erased from this device')
  }

  $effect(() => {
    // recompute report slice when range changes
    void reportRange
    if (tab === 'reports' || tab === 'today') refresh()
  })

  onMount(async () => {
    const s = await invoke<{ paused: boolean; excluded: string[] }>('tracker_status')
    paused = s.paused
    excludedText = s.excluded.join('\n')
    await refresh()
    poll = setInterval(refresh, 5000)
  })

  onDestroy(() => clearInterval(poll))
</script>

<div class="app">
  <header class="topbar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <span class="logo" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
          <circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>
        </svg>
      </span>
      <div class="brand-text" data-tauri-drag-region>
        <h1>FlowLog</h1>
        <p>local activity log</p>
      </div>
    </div>

    <nav class="tabs" aria-label="Views">
      <button class="tab" class:active={tab === 'today'} onclick={() => (tab = 'today')}>Today</button>
      <button class="tab" class:active={tab === 'reports'} onclick={() => (tab = 'reports')}>Reports</button>
      <button class="tab" class:active={tab === 'settings'} onclick={() => (tab = 'settings')}>Settings</button>
    </nav>

    <button class="track-pill" class:paused={paused} onclick={togglePaused} title={paused ? 'Resume tracking' : 'Pause tracking'}>
      <span class="dot"></span>{paused ? 'Paused' : 'Recording'}
    </button>
    <button class="icon-btn" onclick={cycleTheme} title="Cycle theme" aria-label="Cycle theme">
      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="4.5"/><path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M4.6 4.6l1.8 1.8M17.6 17.6l1.8 1.8M4.6 19.4l1.8-1.8M17.6 6.4l1.8-1.8"/></svg>
    </button>
  </header>

  {#if unsupportedReason}
    <div class="banner warn-banner" role="status">
      <strong>Tracking unavailable.</strong> {unsupportedReason} Your existing data below is from previous sessions.
    </div>
  {/if}

  <main class="content">
    {#if tab === 'today'}
      <section class="stat-row">
        <div class="stat card"><span class="stat-label">Tracked today</span><span class="stat-value">{formatDuration(todayTotal)}</span></div>
        <div class="stat card"><span class="stat-label">Most used</span><span class="stat-value">{todayMost ?? '—'}</span></div>
        <div class="stat card"><span class="stat-label">App switches</span><span class="stat-value">{todaySwitches}</span></div>
        <div class="stat card"><span class="stat-label">Now</span><span class="stat-value small">{currentApp ?? (paused ? 'paused' : '—')}</span></div>
      </section>

      <div class="panes">
        <section class="card pane">
          <h2>Top apps <span class="dim">· today</span></h2>
          {#if todayUsage.length === 0}
            <p class="dim empty">No data yet. FlowLog records locally while you use your computer (X11 required on Linux).</p>
          {:else}
            <div class="bar-list">
              {#each todayUsage.slice(0, 8) as u (u.process)}
                <div class="bar-row">
                  <span class="bar-name" title={u.process}>{u.app}</span>
                  <div class="bar-track"><div class="bar-fill accent" style:width="{(u.seconds / (todayUsage[0]?.seconds || 1)) * 100}%"></div></div>
                  <span class="bar-meta mono">{formatDuration(u.seconds)}</span>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <section class="card pane">
          <h2>Timeline <span class="dim">· 24h</span></h2>
          <div class="timeline" aria-hidden="true">
            {#each timelineBuckets as sec, h}
              <div class="tlot" title="{h}:00 — {formatDuration(sec)}">
                <div class="tbar" style:height="{(sec / maxBucket) * 100}%"></div>
                {#if h % 3 === 0}<span class="thour">{String(h).padStart(2, '0')}</span>{/if}
              </div>
            {/each}
          </div>
          <h2 class="cat-title">Categories <span class="dim">· today</span></h2>
          <div class="bar-list">
            {#each todayCats as c (c.category)}
              <div class="bar-row">
                <span class="bar-name">{c.category}</span>
                <div class="bar-track"><div class="bar-fill cat" style:width="{(c.seconds / (todayCats[0]?.seconds || 1)) * 100}%"></div></div>
                <span class="bar-meta mono">{formatDuration(c.seconds)}</span>
              </div>
            {/each}
            {#if todayCats.length === 0}<p class="dim empty">Categories appear once data exists.</p>{/if}
          </div>
        </section>
      </div>

    {:else if tab === 'reports'}
      <div class="report-head">
        <h2>Reports</h2>
        <div class="range-switch" role="group" aria-label="Range">
          {#each [['7d', '7 days'], ['30d', '30 days'], ['month', 'This month']] as [key, label] (key)}
            <button class="chip-btn" class:active={reportRange === key} onclick={() => (reportRange = key as ReportRange)}>{label}</button>
          {/each}
        </div>
        <div class="export-actions">
          <button class="btn btn-ghost" onclick={() => exportReport('csv')}>Export CSV</button>
          <button class="btn btn-ghost" onclick={() => exportReport('json')}>Export JSON</button>
        </div>
      </div>
      <p class="dim report-summary">Total {formatDuration(reportTotal)} tracked over {reportEvents.length} recorded samples. Nothing left this device except the export you just saved.</p>

      <div class="panes">
        <section class="card pane">
          <h2>By app</h2>
          {#if reportUsage.length === 0}
            <p class="dim empty">No samples in this range yet.</p>
          {:else}
            <table class="usage-table">
              <thead><tr><th>#</th><th>App</th><th>Time</th><th>Share</th></tr></thead>
              <tbody>
                {#each reportUsage.slice(0, 15) as u, i (u.process)}
                  <tr>
                    <td class="dim">{i + 1}</td>
                    <td title={u.process}>{u.app}</td>
                    <td class="mono">{formatDuration(u.seconds)}</td>
                    <td class="mono">{reportTotal ? Math.round((u.seconds / reportTotal) * 100) : 0}%</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </section>
        <section class="card pane">
          <h2>By category</h2>
          <div class="bar-list">
            {#each reportCats as c (c.category)}
              <div class="bar-row">
                <span class="bar-name">{c.category}</span>
                <div class="bar-track"><div class="bar-fill cat" style:width="{maxReportSec ? (c.seconds / (reportCats[0]?.seconds || 1)) * 100 : 0}%"></div></div>
                <span class="bar-meta mono">{formatDuration(c.seconds)}</span>
              </div>
            {/each}
            {#if reportCats.length === 0}<p class="dim empty">No data yet.</p>{/if}
          </div>
          <h2 class="cat-title">Custom category rules</h2>
          <p class="dim rules-hint">Map a process keyword to a category (e.g. “blender” → Media). Applied on top of built-ins.</p>
          <div class="rule-add">
            <input class="input slim" placeholder="keyword, e.g. blender" bind:value={ruleKw} onkeydown={(e) => e.key === 'Enter' && addRule()} />
            <select class="input slim" bind:value={ruleCat}>
              {#each CATEGORIES as c}<option value={c}>{c}</option>{/each}
            </select>
            <button class="btn btn-ghost btn-sm" onclick={addRule} disabled={!ruleKw.trim()}>Add</button>
          </div>
          <div class="rule-list">
            {#each userRules as [kw, cat], i (kw + i)}
              <span class="rule-chip">{kw} → {cat}<button class="rule-del" onclick={() => removeRule(i)} aria-label="Remove rule">✕</button></span>
            {/each}
            {#if userRules.length === 0}<p class="dim empty">No custom rules yet.</p>{/if}
          </div>
        </section>
      </div>

    {:else if tab === 'settings'}
      <div class="settings-grid">
        <section class="card pane">
          <h2>Tracking</h2>
          <div class="track-row">
            <div>
              <p class="set-title">Activity tracking is {paused ? 'paused' : 'recording'}</p>
              <p class="dim set-sub">{trackedEvents} samples recorded on this device. 5-second polling; only the frontmost app is read, never your screen or keys.</p>
            </div>
            <button class="btn {paused ? 'btn-primary' : 'btn-ghost'}" onclick={togglePaused}>{paused ? 'Resume' : 'Pause'}</button>
          </div>
          <h2 class="cat-title">Excluded apps</h2>
          <p class="dim rules-hint">One process keyword per line (e.g. “firefox”, “game”). Matching apps are never recorded.</p>
          <textarea class="input excl" rows="5" placeholder="firefox&#10;steam&#10;passwordmanager" bind:value={excludedText}></textarea>
          <button class="btn btn-ghost btn-sm" onclick={saveExcluded}>Save exclusions</button>
        </section>

        <section class="card pane danger-card">
          <h2>Your data</h2>
          <ul class="privacy-list">
            <li>Stored only in this device’s app-data folder (JSONL).</li>
            <li>No screenshots, no keystrokes, no network, no telemetry, no account.</li>
            <li>Exports you make are written to your Downloads folder and nothing else.</li>
            <li>Uninstalling FlowLog removes the tracker; erase data below to clear it sooner.</li>
          </ul>
          {#if !eraseArmed}
            <button class="btn btn-danger" onclick={eraseAll}>Erase all tracked data…</button>
          {:else}
            <div class="erase-confirm">
              <p class="danger-text">This permanently deletes {trackedEvents} recorded samples from this device.</p>
              <div class="erase-actions">
                <button class="btn btn-ghost btn-sm" onclick={() => (eraseArmed = false)}>Keep data</button>
                <button class="btn btn-danger btn-sm" onclick={eraseAll}>Erase everything</button>
              </div>
            </div>
          {/if}
        </section>
      </div>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="dim">FlowLog v0.1.0 · all data stays on this device</span>
    <span class="dim">{trackedEvents} samples · {paused ? 'paused' : 'recording'}</span>
  </footer>

  {#if toast}<div class="toast">{toast}</div>{/if}
</div>

<style>
  .app { display: flex; flex-direction: column; height: 100%; }
  .topbar { display: flex; align-items: center; gap: 1rem; padding: .5rem .9rem; background: var(--bg-elevated); border-bottom: 1px solid var(--border); user-select: none; }
  .brand { display: flex; align-items: center; gap: .55rem; }
  .logo { display: grid; place-items: center; width: 34px; height: 34px; border-radius: var(--radius-sm); background: var(--accent-soft); color: var(--accent); }
  .brand-text h1 { font-size: .92rem; margin: 0; }
  .brand-text p { margin: 0; font-size: .66rem; color: var(--fg-dim); text-transform: uppercase; letter-spacing: .12em; }
  .tabs { display: flex; gap: .25rem; margin-left: 1rem; background: var(--bg-inset); border-radius: var(--radius-sm); padding: .2rem; }
  .tab { font: inherit; font-size: .84rem; border: none; background: transparent; color: var(--fg-dim); padding: .32rem .85rem; border-radius: calc(var(--radius-sm) - 2px); cursor: pointer; }
  .tab.active { background: var(--bg-elevated); color: var(--fg); box-shadow: var(--shadow); }
  .track-pill { display: flex; align-items: center; gap: .4rem; margin-left: auto; font: inherit; font-size: .8rem; font-weight: 650; color: var(--accent); background: var(--accent-soft); border: none; border-radius: 999px; padding: .3rem .8rem; cursor: pointer; }
  .track-pill .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--ok); }
  .track-pill.paused { color: var(--fg-dim); background: var(--bg-inset); }
  .track-pill.paused .dot { background: var(--fg-dim); }

  .warn-banner { padding: .5rem 1rem; background: var(--warn-soft); color: var(--warn); border-bottom: 1px solid var(--border); font-size: .83rem; }
  .content { flex: 1; min-height: 0; overflow-y: auto; padding: 1rem; }

  .stat-row { display: grid; grid-template-columns: repeat(4, 1fr); gap: .6rem; margin-bottom: .7rem; }
  .stat { padding: .6rem .8rem; display: flex; flex-direction: column; gap: .2rem; }
  .stat-label { font-size: .68rem; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; color: var(--fg-dim); }
  .stat-value { font-size: 1.3rem; font-weight: 700; }
  .stat-value.small { font-size: .95rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .panes { display: grid; grid-template-columns: 1fr 1fr; gap: .7rem; }
  .pane { padding: .8rem .9rem; }
  .pane h2 { font-size: .9rem; margin: 0 0 .55rem; }
  .cat-title { margin-top: .8rem; }
  .empty { font-size: .83rem; padding: .6rem 0; }

  .bar-list { display: flex; flex-direction: column; gap: .3rem; }
  .bar-row { display: flex; align-items: center; gap: .5rem; }
  .bar-name { width: 32%; min-width: 0; font-size: .8rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .bar-track { flex: 1; height: 14px; background: var(--bg-inset); border-radius: 4px; overflow: hidden; }
  .bar-fill { height: 100%; border-radius: 4px; }
  .bar-fill.accent { background: var(--accent); }
  .bar-fill.cat { background: var(--accent); opacity: .75; }
  .bar-meta { font-size: .7rem; color: var(--fg-dim); min-width: 3.2em; text-align: right; }

  .timeline { display: flex; align-items: flex-end; gap: 3px; height: 96px; padding: .2rem .1rem 1.1rem; border-bottom: 1px solid var(--border); }
  .tlot { flex: 1; position: relative; height: 100%; display: flex; align-items: flex-end; }
  .tbar { width: 100%; min-height: 2px; background: var(--accent); border-radius: 2px 2px 0 0; opacity: .85; }
  .thour { position: absolute; bottom: -1.05rem; left: 0; font-size: .58rem; color: var(--fg-dim); }

  .report-head { display: flex; align-items: center; gap: .8rem; margin-bottom: .25rem; }
  .report-head h2 { font-size: 1rem; margin: 0; }
  .range-switch { display: flex; gap: .3rem; }
  .chip-btn { font: inherit; font-size: .76rem; border: 1px solid var(--border); background: transparent; color: var(--fg-dim); border-radius: 999px; padding: .2rem .6rem; cursor: pointer; }
  .chip-btn.active { background: var(--accent-soft); color: var(--accent); border-color: transparent; }
  .export-actions { margin-left: auto; display: flex; gap: .4rem; }
  .report-summary { font-size: .8rem; margin: .1rem 0 .6rem; }
  .usage-table { width: 100%; border-collapse: collapse; font-size: .82rem; }
  .usage-table th { text-align: left; font-size: .68rem; letter-spacing: .06em; text-transform: uppercase; color: var(--fg-dim); padding: .3rem .4rem; border-bottom: 1px solid var(--border); }
  .usage-table td { padding: .3rem .4rem; border-bottom: 1px solid var(--border); }

  .rules-hint { font-size: .76rem; margin: 0 0 .4rem; }
  .rule-add { display: flex; gap: .35rem; margin-bottom: .4rem; }
  .input.slim { padding: .32rem .5rem; font-size: .8rem; }
  .rule-list { display: flex; flex-wrap: wrap; gap: .3rem; }
  .rule-chip { display: inline-flex; align-items: center; gap: .35rem; font-size: .74rem; background: var(--bg-inset); border-radius: 999px; padding: .18rem .3rem .18rem .55rem; }
  .rule-del { background: transparent; border: none; color: var(--fg-dim); cursor: pointer; }

  .settings-grid { display: grid; grid-template-columns: 1fr 1fr; gap: .7rem; }
  .track-row { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
  .set-title { margin: 0 0 .15rem; font-weight: 650; font-size: .9rem; }
  .set-sub { margin: 0; font-size: .78rem; max-width: 46ch; }
  .excl { width: 100%; resize: vertical; font-size: .82rem; }
  .privacy-list { font-size: .84rem; line-height: 1.55; padding-left: 1.1rem; margin: .2rem 0 .7rem; }
  .danger-card { border-color: var(--danger-soft); }
  .danger-text { color: var(--fg); font-size: .84rem; }
  .erase-actions { display: flex; gap: .4rem; }

  .statusbar { display: flex; justify-content: space-between; padding: .34rem .9rem; font-size: .7rem; background: var(--bg-elevated); border-top: 1px solid var(--border); }
  @media (max-width: 980px) {
    .stat-row { grid-template-columns: repeat(2, 1fr); }
    .panes, .settings-grid { grid-template-columns: 1fr; }
  }
</style>
