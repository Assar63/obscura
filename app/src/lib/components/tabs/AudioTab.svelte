<script lang="ts">
  // The camera's microphones, and OBSBOT's Vox SE wireless microphones
  // (Tiny 3 series: two slots, TX1 and TX2). The slots come from the device
  // store's regular poll.
  import { onDestroy } from "svelte";
  import type { MicSlot } from "../../api";
  import { device } from "../../device.svelte";
  import Card from "../ui/Card.svelte";
  import ConfirmDialog from "../ui/ConfirmDialog.svelte";
  import Feature from "../controls/Feature.svelte";

  const PAIR_TIMEOUT_MS = 90_000;

  // Pairing in progress: slot, a timeout, and how it ended.
  let pairing = $state<number | null>(null);
  let pairTimer: ReturnType<typeof setTimeout> | undefined;
  let pairResult = $state<{ slot: number; ok: boolean } | null>(null);

  let forgetSlot = $state(1);
  let confirmForget = $state(false);

  // Done as soon as the camera reports the slot online.
  $effect(() => {
    if (pairing !== null && device.mics?.find((m) => m.slot === pairing)?.connected) {
      clearTimeout(pairTimer);
      pairResult = { slot: pairing, ok: true };
      pairing = null;
    }
  });
  onDestroy(() => clearTimeout(pairTimer));

  function pair(slot: number) {
    pairResult = null;
    pairing = slot;
    device.set(`mic_pair_tx${slot}`, 1);
    clearTimeout(pairTimer);
    pairTimer = setTimeout(() => {
      if (pairing === null) return;
      device.set("mic_pair_stop", 1);
      pairResult = { slot: pairing, ok: false };
      pairing = null;
    }, PAIR_TIMEOUT_MS);
  }

  function cancel() {
    clearTimeout(pairTimer);
    device.set("mic_pair_stop", 1);
    pairing = null;
  }

  function slotText(m: MicSlot): string {
    if (!m.connected) return "Not connected";
    const parts = ["Connected"];
    if (m.battery !== null) parts.push(`${m.battery}%`);
    if (m.charging) parts.push("charging");
    if (m.muted) parts.push("muted");
    return parts.join(" · ");
  }
</script>

<div class="tab">
  <Card title="Microphone">
    <Feature
      id="mic_capture"
      description="The camera's microphone level in hardware: Teams, OBS and browsers get this. Your system's input volume applies on top."
    />
    <Feature id="mic_level" hideUnsupported />
    <Feature id="disable_microphone" />
    <Feature id="noise_reduction" variant="select" />
    <Feature id="auto_gain" />
    <Feature id="pickup_distance" variant="select" />
    <Feature id="mic_during_sleep" />
  </Card>

  {#if device.supported("voice_track_me")}
    <Card title="Voice Control">
      <p class="hint">Spoken commands the camera listens for (in the language below).</p>
      <Feature id="voice_language" />
      <Feature id="voice_hi_tiny" />
      <Feature id="voice_sleep_tiny" />
      <Feature id="voice_track_me" />
      <Feature id="voice_unlock_me" />
      <Feature id="voice_zoom_in" />
      <Feature id="voice_zoom_out" />
      <Feature id="voice_zoom_factor" variant="select" />
      <Feature id="voice_presets" />
    </Card>
  {/if}

  {#if device.mics}
    <Card title="Wireless Microphones">
      {#each device.mics as m (m.slot)}
        <div class="slot-group">
          <div class="slot">
            <span class="dot" class:on={m.connected}></span>
            <div class="text">
              <span>TX{m.slot}</span>
              <span class="desc">{slotText(m)}</span>
            </div>
            {#if pairing === m.slot}
              <button class="small" onclick={cancel}>Cancel</button>
            {:else if m.connected}
              <button
                class="small ghost"
                disabled={pairing !== null}
                onclick={() => ((forgetSlot = m.slot), (confirmForget = true))}>Forget</button
              >
            {:else}
              <button
                class="small"
                disabled={pairing !== null || !device.supported(`mic_pair_tx${m.slot}`)}
                onclick={() => pair(m.slot)}>Pair</button
              >
            {/if}
          </div>
          {#if m.connected}
            <div class="slot-controls">
              <Feature id={`mic_tx${m.slot}_mute`} label="Mute" />
              <Feature id={`mic_tx${m.slot}_gain`} label="Gain" />
            </div>
          {/if}
        </div>
      {/each}

      {#if pairing !== null}
        <p class="hint">
          Pairing TX{pairing}: take the Vox SE out of its case, then hold its button for about 6 seconds
          until the light flashes fast. Waiting for it to connect…
        </p>
      {:else if pairResult?.ok}
        <p class="hint ok">TX{pairResult.slot} is paired. It reconnects by itself from now on.</p>
      {:else if pairResult}
        <p class="hint fail">
          TX{pairResult.slot} didn't connect within 90 seconds. Make sure the mic's light flashes fast
          (hold the button for 6 s), then try Pair again.
        </p>
      {/if}

      <Feature id="mic_button" variant="select" />
      <Feature id="audio_source" />
      <Feature id="audio_auto_select" />
    </Card>
  {/if}
</div>

<ConfirmDialog
  bind:open={confirmForget}
  message={`Forget the microphone on TX${forgetSlot}? It will need pairing again.`}
  onconfirm={() => device.set(`mic_forget_tx${forgetSlot}`, 1)}
/>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .slot-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .slot-controls {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-left: 18px;
  }
  .slot {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 32px;
  }
  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .desc {
    font-size: 12px;
    color: var(--text-muted);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-dim);
    flex: none;
  }
  .dot.on {
    background: #3ecf6e;
  }
  button.small {
    font-size: 12px;
    padding: 4px 12px;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  .hint.ok {
    color: #3ecf6e;
  }
  .hint.fail {
    color: #f0b44c;
  }
</style>
