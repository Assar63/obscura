<script lang="ts">
  // The Diagnostics window (Ctrl+Shift+D in the main window): the camera's
  // live state and the activity log, beside the main window while you use
  // it. It talks to the backend only; it doesn't load the main window's
  // device store, which would poll the camera a second time.
  import { onDestroy } from "svelte";
  import { api, type LiveStatus, type LogEntry } from "../api";

  const POLL_MS = 1000;
  const KEEP = 500;
  const RAW_KEY = "diagnostics.showTrace";

  let live = $state<LiveStatus | null>(null);
  let liveError = $state<string | null>(null);
  let entries = $state<LogEntry[]>([]);
  let showTrace = $state(readFlag(RAW_KEY));
  let lastSeq = 0;

  const shown = $derived(entries.filter((e) => showTrace || e.level !== "trace").slice().reverse());
  const hasTrace = $derived(entries.some((e) => e.level === "trace"));

  function readFlag(key: string): boolean {
    try {
      return localStorage.getItem(key) === "1";
    } catch {
      return false;
    }
  }

  function setShowTrace(v: boolean) {
    showTrace = v;
    try {
      localStorage.setItem(RAW_KEY, v ? "1" : "0");
    } catch {
      // Not remembered; harmless.
    }
  }

  async function poll() {
    try {
      const fresh = await api.getLog(lastSeq);
      if (fresh.length) {
        lastSeq = fresh[fresh.length - 1].seq;
        entries = [...entries, ...fresh].slice(-KEEP);
      }
    } catch {
      // Backend unreachable; try again next time.
    }
    try {
      live = await api.liveStatus();
      liveError = live ? null : "This camera doesn't report a live status.";
    } catch (e) {
      live = null;
      liveError = String(e) === "no camera open" ? "No camera open in OBSCura." : String(e);
    }
  }

  void poll();
  const timer = setInterval(() => void poll(), POLL_MS);
  onDestroy(() => clearInterval(timer));

  const time = (ms: number) =>
    new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
</script>

<main>
  <section class="live">
    <h2>Camera</h2>
    {#if live}
      <dl>
        <dt>Power</dt>
        <dd>{live.power}</dd>
        <dt>AI mode</dt>
        <dd>{live.ai_mode_label}{live.ai_sub_mode ? ` (sub-mode ${live.ai_sub_mode})` : ""}</dd>
        <dt>Zoom</dt>
        <dd>{live.zoom.toFixed(2)}x</dd>
        <dt>Field of view</dt>
        <dd>{live.fov ?? "–"}</dd>
        <dt>Stream fps</dt>
        <dd>{live.fps}</dd>
        <dt>Status light</dt>
        <dd>level {live.light_level}</dd>
        <dt>Queued events</dt>
        <dd>{live.event_count}</dd>
        <dt>Wireless mic</dt>
        <dd>{live.mic}{live.audio_source === 3 ? " · source: wireless" : ""}</dd>
      </dl>
    {:else}
      <p class="note">{liveError ?? "Reading…"}</p>
    {/if}
  </section>

  <section class="activity">
    <div class="head">
      <h2>Activity</h2>
      {#if hasTrace}
        <label>
          <input type="checkbox" checked={showTrace} onchange={(e) => setShowTrace(e.currentTarget.checked)} />
          Raw frames
        </label>
      {/if}
    </div>
    {#if shown.length}
      <ol class="log">
        {#each shown as e (e.seq)}
          <li class={e.level}>
            <time>{time(e.time_ms)}</time>
            <span>{e.message}</span>
          </li>
        {/each}
      </ol>
    {:else}
      <p class="note">
        Nothing yet. Changes you make in OBSCura, and any the camera ignores, show up here. Start
        OBSCura with OBSCURA_TRACE=1 to also see raw frames.
      </p>
    {/if}
  </section>
</main>

<style>
  main {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px;
    box-sizing: border-box;
    background: var(--bg);
  }
  h2 {
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .live {
    background: var(--card);
    border-radius: var(--radius);
    padding: 10px 12px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0;
    font-size: 13px;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 0;
  }
  .activity {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--card);
    border-radius: var(--radius);
    padding: 10px 12px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .head label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .log {
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .log li {
    display: flex;
    gap: 8px;
  }
  .log time {
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    flex: none;
  }
  .log span {
    overflow-wrap: anywhere;
    user-select: text;
  }
  .log .warn span {
    color: #f0b44c;
  }
  .log .trace span {
    color: var(--text-muted);
    font-family: monospace;
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
