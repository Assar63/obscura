<script lang="ts">
  import { device } from "../device.svelte";
  import Card from "./Card.svelte";
  import Feature from "./Feature.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Toggle from "./Toggle.svelte";
  import { api } from "../api";

  let confirmReset = $state(false);

  // Panel indicator autostart. When on, the indicator also stays running
  // after the app closes.
  let autostart = $state<boolean | null>(null);
  api.getAutostart().then((v) => (autostart = v), () => (autostart = null));

  async function setAutostart(enabled: boolean) {
    autostart = enabled;
    try {
      autostart = await api.setAutostart(enabled);
    } catch (e) {
      device.error = String(e);
      autostart = await api.getAutostart().catch(() => null);
    }
  }
  const hex = (n: number) => n.toString(16).padStart(4, "0");
</script>

<div class="tab">
  <Card title="Audio">
    <Feature id="mic_during_sleep" />
    <Feature id="noise_reduction" variant="select" />
    <Feature id="auto_gain" />
    <Feature id="disable_microphone" />
    <Feature id="pickup_distance" variant="select" />
  </Card>

  <Card title="Device Sleep">
    <Feature id="auto_sleep" />
    {#if device.on("auto_sleep") || !device.supported("auto_sleep")}
      <Feature id="sleep_time" variant="select" />
      <p class="note">
        When the video stream is not turned on, it will enter the sleep mode after the above time.
      </p>
    {/if}
    <Feature id="sleep_background_mirror" />
  </Card>

  <Card title="Indicator Status">
    <Feature id="status_light" />
    <Feature id="status_light_brightness" />
  </Card>

  <Card title="Panel Indicator">
    <div class="row">
      <div class="text">
        <span>Start at login</span>
        <span class="desc">
          Keep the OBSCura indicator in the panel for quick camera controls, even after this
          window is closed.
        </span>
      </div>
      <Toggle checked={autostart ?? false} disabled={autostart === null} onchange={setAutostart} />
    </div>
  </Card>

  <Card title="More Settings">
    <Feature id="gimbal_reverse" />
    <button
      disabled={!device.supported("factory_reset")}
      title={device.features.factory_reset?.reason ?? ""}
      onclick={() => (confirmReset = true)}>Factory Reset</button
    >
  </Card>

  <Card title="Device">
    {#if device.current}
      <dl>
        <dt>Model</dt>
        <dd>{device.current.product ?? device.current.name}</dd>
        <dt>USB ID</dt>
        <dd>{hex(device.current.vendor_id)}:{hex(device.current.product_id)}</dd>
        <dt>USB revision</dt>
        <dd>{device.current.usb_version ?? "–"}</dd>
        <dt>Serial</dt>
        <dd>{device.current.serial ?? "–"}</dd>
        <dt>Node</dt>
        <dd>{device.current.path}</dd>
        <dt>Profile</dt>
        <dd>{device.profileName}</dd>
      </dl>
    {:else}
      <p class="note">No camera connected.</p>
    {/if}
  </Card>
</div>

<ConfirmDialog
  bind:open={confirmReset}
  message="Restore all camera settings to factory defaults?"
  onconfirm={() => device.set("factory_reset", 1)}
/>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .desc {
    font-size: 12px;
    color: var(--text-muted);
  }
  .note {
    margin: -6px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
    margin: 0;
    font-size: 13px;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
