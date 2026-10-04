<script lang="ts">
  // The camera's microphones, and wireless microphones (Vox SE on the
  // Tiny 3). Pairing is experimental: the commands come from OBSBOT's SDK,
  // but none has been seen to start pairing yet (see TODO.md).
  import { onDestroy } from "svelte";
  import { api } from "../api";
  import { device } from "../device.svelte";
  import Card from "./Card.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Feature from "./Feature.svelte";

  const PAIRING = ["mic_pair_tx1", "mic_pair_tx2", "mic_pair_stop", "mic_ble_pairing"];
  const wireless = $derived(device.supported("audio_source") || PAIRING.some((id) => device.supported(id)));

  // The live mic state comes from the status block, not a feature.
  let mic = $state<string | null>(null);
  async function pollMic() {
    if (!wireless || !device.current) return;
    try {
      mic = (await api.liveStatus())?.mic ?? null;
    } catch {
      mic = null;
    }
  }
  void pollMic();
  const timer = setInterval(() => void pollMic(), 2000);
  onDestroy(() => clearInterval(timer));

  let forget = $state<"mic_forget_tx1" | "mic_forget_tx2">("mic_forget_tx1");
  let confirmForget = $state(false);

  function act(id: string) {
    device.set(id, 1);
    setTimeout(() => void pollMic(), 1500);
  }
</script>

<div class="tab">
  <Card title="Microphone">
    <Feature id="disable_microphone" />
    <Feature id="noise_reduction" variant="select" />
    <Feature id="auto_gain" />
    <Feature id="pickup_distance" variant="select" />
    <Feature id="mic_during_sleep" />
  </Card>

  {#if wireless}
    <Card title="Wireless Microphones (experimental)">
      <div class="status">
        <span class="dot" class:on={!!mic && !mic.startsWith("no ")}></span>
        {mic ?? "–"}
      </div>
      <Feature id="audio_source" />
      <Feature id="audio_auto_select" />

      <p class="note">
        Pairing from Linux is experimental. To try: set Audio Source to Wireless Mic, hold the Vox SE's
        button for 6 s until its light flashes fast, then press Pair TX1. Watch the status above and
        the Diagnostics window (Ctrl+Shift+D). If nothing works, pair once with OBSBOT Center on
        Windows or macOS; the mic reconnects by itself afterwards.
      </p>
      <div class="buttons">
        <button disabled={!device.supported("mic_pair_tx1")} onclick={() => act("mic_pair_tx1")}>Pair TX1</button>
        <button disabled={!device.supported("mic_pair_tx2")} onclick={() => act("mic_pair_tx2")}>Pair TX2</button>
        <button disabled={!device.supported("mic_pair_stop")} onclick={() => act("mic_pair_stop")}>Stop Pairing</button>
        <button
          disabled={!device.supported("mic_ble_pairing")}
          title="setBlePairingEnable in OBSBOT's SDK: the camera's Bluetooth radio"
          onclick={() => act("mic_ble_pairing")}>Bluetooth Pairing Mode</button
        >
        <button class="ghost" disabled={!device.supported("mic_forget_tx1")} onclick={() => ((forget = "mic_forget_tx1"), (confirmForget = true))}
          >Forget TX1</button
        >
        <button class="ghost" disabled={!device.supported("mic_forget_tx2")} onclick={() => ((forget = "mic_forget_tx2"), (confirmForget = true))}
          >Forget TX2</button
        >
      </div>
    </Card>
  {/if}
</div>

<ConfirmDialog
  bind:open={confirmForget}
  message={`Forget the pairing of ${forget === "mic_forget_tx2" ? "TX2" : "TX1"}? It will need pairing again.`}
  onconfirm={() => act(forget)}
/>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
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
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  .buttons {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .buttons button {
    font-size: 12px;
    padding: 5px 8px;
  }
</style>
