<script lang="ts">
  import { device } from "../../device.svelte";
  import { preview as stream } from "../../preview.svelte";
  import Feature from "../controls/Feature.svelte";

  let { preview }: { preview: boolean } = $props();

  const sleeping = $derived(device.on("sleep"));
  const path = $derived(device.current?.path ?? null);
  const wanted = $derived(preview && !!path && !sleeping);

  let canvas: HTMLCanvasElement | null = $state(null);
  $effect(() => stream.attach(canvas));

  // (Re)start whenever the camera, the requested format or visibility
  // changes; stop when the preview isn't wanted.
  $effect(() => {
    const p = path;
    const key = `${stream.resolution}/${stream.fps}`;
    if (wanted && p) {
      void key;
      void stream.start(p);
      return () => void stream.stop();
    }
  });
  const others = $derived(device.cameras.filter((c) => !c.is_obsbot));
</script>

<div class="preview">
  {#if !device.current}
    <div class="empty">
      <h2>No OBSBOT camera found</h2>
      <p>Plug the camera in over USB, then rescan.</p>
      <button onclick={() => device.refreshCameras()}>Rescan</button>
      {#if others.length}
        <p class="muted">Or try the standard UVC controls on another camera:</p>
        <div class="others">
          {#each others as c (c.path)}
            <button class="ghost" onclick={() => device.open(c.path)}>{c.product ?? c.name}</button>
          {/each}
        </div>
      {/if}
    </div>
  {:else if sleeping}
    <div class="sleeping">
      <h2>Device is sleeping</h2>
      <button class="resume" onclick={() => device.set("sleep", 0)}>Resume</button>
      <div class="mic"><Feature id="mic_during_sleep" label="MIC Status During Sleep" /></div>
    </div>
  {:else if preview}
    <canvas bind:this={canvas} class:hidden={!stream.format} class:mirrored={stream.mirror}></canvas>
    {#if !stream.format}
      <div class="placeholder overlay">
        {#if stream.error}
          <span>{stream.error}</span>
          <button onclick={() => path && stream.start(path)}>Retry</button>
        {:else}
          <span class="muted">Starting preview…</span>
        {/if}
      </div>
    {:else}
      <span class="badge">{stream.format.width}×{stream.format.height} · {stream.format.fps} fps</span>
    {/if}
  {:else}
    <div class="placeholder"><span class="muted">Preview closed</span></div>
  {/if}
  {#if device.error}
    <div class="error" role="alert">
      {device.error}
      <button class="ghost" onclick={() => (device.error = null)}>✕</button>
    </div>
  {/if}
</div>

<style>
  .preview {
    position: relative;
    overflow: hidden;
    flex: 1;
    background: #000;
    display: grid;
    place-items: center;
    min-height: 0;
  }
  .empty,
  .sleeping,
  .placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    text-align: center;
  }
  h2 {
    font-weight: 500;
    margin: 0 0 8px;
  }
  .muted {
    color: var(--text-muted);
    font-size: 13px;
  }
  .others {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: center;
  }
  .resume {
    min-width: 140px;
    padding: 8px;
  }
  .mic {
    width: 280px;
    margin-top: 24px;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  canvas.hidden {
    visibility: hidden;
  }
  .overlay {
    position: relative;
  }
  .badge {
    position: absolute;
    right: 10px;
    bottom: 8px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.6);
    background: rgba(0, 0, 0, 0.45);
    padding: 2px 6px;
    border-radius: 4px;
  }
  .error {
    position: absolute;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    background: #5a1220;
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm);
    padding: 6px 6px 6px 12px;
    display: flex;
    gap: 8px;
    align-items: center;
    max-width: 80%;
    font-size: 13px;
  }
  .error button {
    padding: 2px 6px;
  }
  .mirrored {
    transform: scaleX(-1);
  }
</style>
