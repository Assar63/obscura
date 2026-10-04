<script lang="ts">
  // The camera's microphones, and OBSBOT's Vox SE wireless microphones
  // (Tiny 3 series: two slots, TX1 and TX2).
  import { onDestroy } from "svelte";
  import { api, type MicSlot } from "../api";
  import { device } from "../device.svelte";
  import Card from "./Card.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Feature from "./Feature.svelte";

  const POLL_MS = 2000;
  const PAIR_TIMEOUT_MS = 90_000;

  let mics = $state<MicSlot[] | null>(null);
  // Pairing in progress: slot, start time, and how it ended.
  let pairing = $state<number | null>(null);
  let pairStart = 0;
  let pairResult = $state<{ slot: number; ok: boolean } | null>(null);

  let forgetSlot = $state(1);
  let confirmForget = $state(false);

  async function poll() {
    if (!device.current) {
      mics = null;
      return;
    }
    try {
      mics = await api.wirelessMics();
    } catch {
      mics = null;
    }
    if (pairing !== null) {
      const slot = mics?.find((m) => m.slot === pairing);
      if (slot?.connected) {
        pairResult = { slot: pairing, ok: true };
        pairing = null;
      } else if (Date.now() - pairStart > PAIR_TIMEOUT_MS) {
        device.set("mic_pair_stop", 1);
        pairResult = { slot: pairing, ok: false };
        pairing = null;
      }
    }
  }

  void poll();
  const timer = setInterval(() => void poll(), POLL_MS);
  onDestroy(() => clearInterval(timer));

  function pair(slot: number) {
    pairResult = null;
    pairing = slot;
    pairStart = Date.now();
    device.set(`mic_pair_tx${slot}`, 1);
  }

  function cancel() {
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

  {#if mics}
    <Card title="Wireless Microphones">
      {#each mics as m (m.slot)}
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
