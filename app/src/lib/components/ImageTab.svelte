<script lang="ts">
  import { device } from "../device.svelte";
  import Card from "./Card.svelte";
  import Feature from "./Feature.svelte";
  import Joystick from "./Joystick.svelte";
  import Segmented from "./Segmented.svelte";

  // Gimbal speed only scales joystick moves; OBSBOT Center sends nothing to
  // the camera for it. Remembered per machine.
  const SPEEDS = [
    { value: 0, label: "Slow" },
    { value: 1, label: "Med." },
    { value: 2, label: "Fast" },
  ];
  const SPEED_STEP = [0.008, 0.02, 0.04];
  let speed = $state(loadSpeed());

  function loadSpeed(): number {
    try {
      const v = Number(localStorage.getItem("gimbalSpeed"));
      return v >= 0 && v <= 2 ? v : 1;
    } catch {
      return 1;
    }
  }

  function setSpeed(v: number) {
    speed = v;
    try {
      localStorage.setItem("gimbalSpeed", String(v));
    } catch {
      // storage unavailable; keep it for this session only
    }
  }

  function resetView() {
    if (device.supported("gimbal_reset")) {
      device.set("gimbal_reset", 1);
      return;
    }
    // Fall back to centring with the standard pan/tilt/zoom controls.
    for (const id of ["pan", "tilt"]) if (device.supported(id)) device.set(id, 0);
    const zoom = device.features.zoom;
    if (zoom?.supported && zoom.kind.type === "range") device.set("zoom", zoom.kind.min);
  }

  const IMAGE = ["brightness", "contrast", "saturation", "sharpness", "hue"];

  /** Show a dependent control when its parent is in `state`, or when the
   * parent is unsupported (so the full layout is still visible). */
  function when(parent: string, state: boolean): boolean {
    return !device.supported(parent) || device.on(parent) === state;
  }

  const gimbalSupported = $derived(device.supported("pan") || device.supported("tilt"));

  function joystick(x: number, y: number) {
    device.nudge("pan", x * SPEED_STEP[speed]);
    device.nudge("tilt", y * SPEED_STEP[speed]);
  }
</script>

<div class="tab">
  <Card title="Preset">
    <div class="presets">
      {#each [1, 2, 3] as n (n)}
        <button disabled title="Preset {n}: pending USB captures">Add</button>
      {/each}
    </div>
  </Card>

  <Card
    title="View and Gimbal"
    onreset={device.supported("gimbal_reset") || gimbalSupported ? resetView : undefined}
  >
    <div class="gimbal">
      <div class="gimbal-speed">
        <span class="label" class:dim={!gimbalSupported}>Gimbal Speed</span>
        <Segmented options={SPEEDS} value={speed} disabled={!gimbalSupported} onchange={setSpeed} />
      </div>
      <Joystick disabled={!gimbalSupported} onmove={joystick} />
    </div>
    <Feature id="zoom" />
  </Card>

  <Card title="Image Adj.">
    <Feature id="hdr" />
    <Feature id="auto_focus" />
    {#if when("auto_focus", true)}<Feature id="auto_focus_mode" />{/if}
    {#if when("auto_focus", false)}<Feature id="focus" hideUnsupported={device.supported("auto_focus")} />{/if}
    <Feature id="auto_exposure" />
    {#if when("auto_exposure", true)}
      <Feature id="auto_exposure_mode" label="Auto Exposure Mode" />
      <Feature id="exposure_compensation" />
    {/if}
    {#if when("auto_exposure", false)}
      <Feature id="exposure" hideUnsupported={device.supported("auto_exposure")} />
      <Feature id="gain" hideUnsupported />
    {/if}
    <Feature id="anti_flicker" />
    <Feature id="auto_white_balance" />
    <Feature id="white_balance_temperature" />
  </Card>

  <Card title="Image" onreset={() => device.resetToDefault(IMAGE)}>
    <Feature id="brightness" hideUnsupported />
    <Feature id="contrast" />
    <Feature id="saturation" />
    <Feature id="sharpness" />
    <Feature id="hue" />
  </Card>
</div>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .presets {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .gimbal {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .gimbal-speed {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .gimbal-speed :global(.segmented button) {
    padding: 3px 9px;
  }
  .label.dim {
    color: var(--text-muted);
  }
</style>
