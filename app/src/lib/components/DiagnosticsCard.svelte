<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, type LiveStatus, type LogEntry } from "../api";
  import { device } from "../device.svelte";
  import Card from "./Card.svelte";

  // What the camera is doing right now, and what the app has asked of it:
  // settings written, changes the camera ignored, and errors. Raw frames
  // are only logged when the app is started with OBSCURA_TRACE=1.
  const POLL_MS = 2000;
  const KEEP = 200;

  let open = $state(false);
  let live = $state<LiveStatus | null>(null);
  let entries = $state<LogEntry[]>([]);
  let showTrace = $state(false);
  let lastSeq = 0;

  const shown = $derived(
    entries.filter((e) => showTrace || e.level !== "trace").slice().reverse(),
  );
  const hasTrace = $derived(entries.some((e) => e.level === "trace"));

  const aiLabel = $derived.by(() => {
    if (!live) return "–";
    if (live.ai_mode === 6) return "switching…";
    const kind = device.features.ai_mode?.kind;
    const opt = kind?.type === "choice" ? kind.options.find((o) => o.value === live!.ai_mode) : null;
    return opt?.label ?? String(live.ai_mode);
  });

  async function poll() {
    try {
      const fresh = await api.getLog(lastSeq);
      if (fresh.length) {
        lastSeq = fresh[fresh.length - 1].seq;
        entries = [...entries, ...fresh].slice(-KEEP);
      }
    } catch {
      // The log lives in the backend; nothing to show if it's unreachable.
    }
    if (!open || !device.current) {
      live = null;
      return;
    }
    try {
      live = await api.liveStatus();
    } catch {
      live = null;
    }
  }

  void poll();
  const timer = setInterval(() => void poll(), POLL_MS);
  onDestroy(() => clearInterval(timer));

  const time = (ms: number) =>
    new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
</script>

<Card title="Diagnostics" bind:open>
  {#if live}
    <dl>
      <dt>Power</dt>
      <dd>{live.power}</dd>
      <dt>AI mode</dt>
      <dd>{aiLabel}{live.ai_sub_mode ? ` (sub-mode ${live.ai_sub_mode})` : ""}</dd>
      <dt>Zoom</dt>
      <dd>{live.zoom.toFixed(2)}x</dd>
      <dt>Field of view</dt>
      <dd>{live.fov ?? "–"}</dd>
      <dt>Stream fps</dt>
      <dd>{live.fps}</dd>
      <dt>Status light</dt>
      <dd>level {live.light_level}</dd>
      <dt>Events</dt>
      <dd>{live.event_count}</dd>
    </dl>
  {:else if device.current}
    <p class="note">This camera doesn't report a live status.</p>
  {/if}

  <div class="loghead">
    <span>Activity</span>
    {#if hasTrace}
      <label><input type="checkbox" bind:checked={showTrace} /> Raw frames</label>
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
    <p class="note">Nothing yet. Changes you make, and any the camera ignores, show up here.</p>
  {/if}
</Card>

<style>
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
  .loghead {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 12px;
    color: var(--text-muted);
  }
  .loghead label {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .log {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 220px;
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
