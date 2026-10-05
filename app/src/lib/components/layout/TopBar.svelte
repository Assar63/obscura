<script lang="ts">
  import { device } from "../../device.svelte";
  import Icon from "../ui/Icon.svelte";
  import Popover from "../ui/Popover.svelte";
  import Feature from "../controls/Feature.svelte";
  import Toggle from "../ui/Toggle.svelte";
  import { preview as stream } from "../../preview.svelte";

  let { preview = $bindable(true) }: { preview?: boolean } = $props();

  let gestureOpen = $state(false);
  let mirrorOpen = $state(false);

  const sleeping = $derived(device.on("sleep"));
  const sharing = $derived(!!stream.share?.running);
  const cameraLabel = (c: { product: string | null; name: string; is_obsbot: boolean }) =>
    (c.product ?? c.name) + (c.is_obsbot ? "" : " (generic)");
</script>

<header class="topbar">
  <div class="device">
    <div class="cam-icon"><Icon name="camera" size={26} /></div>
    <div class="device-controls">
      <div class="select-row">
        <select
          value={device.current?.path ?? ""}
          onchange={(e) => device.open((e.currentTarget as HTMLSelectElement).value)}
        >
          {#if !device.current}<option value="" disabled>No camera</option>{/if}
          {#each device.cameras as c (c.path)}
            <option value={c.path}>{cameraLabel(c)}</option>
          {/each}
        </select>
        <button class="ghost icon" title="Rescan cameras" onclick={() => device.refreshCameras()}>
          <Icon name="refresh" size={15} />
        </button>
      </div>
      <div class="quick">
        <Popover bind:open={gestureOpen}>
          {#snippet trigger()}
            <button
              class="quick-btn"
              class:active={device.on("gesture_control")}
              title="Gesture Control"
              onclick={() => (gestureOpen = !gestureOpen)}><Icon name="hand" size={15} /></button
            >
          {/snippet}
          <div class="pop">
            <Feature id="gesture_control" />
            {#if device.on("gesture_control") || !device.supported("gesture_control")}
              <div class="sub">
                <Feature id="gesture_locked_target" description="Turn on/off Tracking" />
                <Feature id="gesture_zoom" description="Execute zoom in/out" />
                <Feature id="gesture_zoom_factor" />
                <Feature id="gesture_dynamic_zoom" description="Zoom in/out with hands" />
                <Feature id="gesture_direction_flip" />
              </div>
            {/if}
          </div>
        </Popover>
        <Popover bind:open={mirrorOpen}>
          {#snippet trigger()}
            <button
              class="quick-btn"
              class:active={device.on("mirror_image") || stream.mirror || sharing}
              title="Mirror and sharing"
              onclick={() => (mirrorOpen = !mirrorOpen)}><Icon name="mirror" size={15} /></button
            >
          {/snippet}
          <div class="pop">
            <Feature
              id="mirror_image"
              description="Mirror the video directly on the device side (effective for all software that accesses the camera feed)"
            />
            <div class="row">
              <div class="text">
                <span class="label">Mirror Preview</span>
                <span class="desc">Only OBSCura's preview, like a mirror; other apps get the normal picture</span>
              </div>
              <Toggle checked={stream.mirror} onchange={() => stream.toggleMirror()} />
            </div>
            <div class="row">
              <div class="text">
                <span class="label">Share as "OBSCura Camera"</span>
                <span class="desc">
                  {sharing
                    ? "Other apps can pick \"OBSCura Camera\"; keeps running when this window closes"
                    : "Let OBS, Teams or a browser use the camera while the preview runs"}
                </span>
              </div>
              <Toggle
                checked={sharing}
                disabled={!device.current}
                onchange={() => device.current && stream.toggleShare(device.current.path)}
              />
            </div>
          </div>
        </Popover>
      </div>
    </div>
    <button
      class="sleep"
      class:primary={sleeping}
      disabled={!device.supported("sleep")}
      title={device.features.sleep?.reason ?? ""}
      onclick={() => device.set("sleep", sleeping ? 0 : 1)}
    >
      <Icon name="power" size={14} />
      {sleeping ? "Resume" : "Sleep"}
    </button>
    <button class="ghost preview-btn" onclick={() => (preview = !preview)} disabled={!device.current}>
      <Icon name="video" size={14} />
      {preview ? "Close Preview" : "Open Preview"}
    </button>
  </div>
  <div class="hint">
    {#if device.current}
      {#if sharing}
        <span class="muted">Sharing: in other apps,</span>
        select <strong>OBSCura Camera</strong>.
      {:else}
        <span class="muted">Other apps can use the camera directly:</span>
        select <strong>{device.current.product ?? device.current.name}</strong>.
      {/if}
      Settings made here apply on the camera itself.
    {:else}
      <span class="muted">Connect an OBSBOT camera over USB.</span>
    {/if}
  </div>
</header>

<style>
  .topbar {
    display: flex;
    gap: 16px;
    align-items: stretch;
    justify-content: space-between;
    padding: 8px 12px;
  }
  .device {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .cam-icon {
    color: var(--text-muted);
  }
  .device-controls {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .select-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .select-row select {
    appearance: none;
    background: transparent
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%238d8e95' stroke-width='2.5'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E")
      no-repeat right 6px center;
    border: 0;
    font-size: 15px;
    padding: 4px 26px 4px 6px;
    max-width: 240px;
  }
  .select-row select:hover {
    background-color: var(--control);
  }
  .icon {
    padding: 4px;
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  .quick {
    display: flex;
    gap: 4px;
  }
  .quick-btn {
    padding: 2px 4px;
    background: transparent;
    color: var(--text-muted);
    display: grid;
    place-items: center;
  }
  .quick-btn.active {
    background: var(--accent);
    color: #fff;
  }
  .pop {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 280px;
  }
  /* Same layout as a Feature row. */
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .desc {
    font-size: 12px;
    color: var(--text-muted);
  }
  .sub {
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-top: 1px solid var(--border);
    padding-top: 12px;
  }
  .sleep,
  .preview-btn {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .hint {
    background: var(--surface);
    border-radius: var(--radius-sm);
    padding: 8px 12px;
    font-size: 12px;
    max-width: 300px;
    align-self: center;
  }
  .muted {
    color: var(--text-muted);
  }
</style>
