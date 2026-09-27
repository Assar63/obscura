<script lang="ts">
  import { device } from "../device.svelte";
  import Card from "./Card.svelte";
  import Feature from "./Feature.svelte";
  import Joystick from "./Joystick.svelte";
  import Segmented from "./Segmented.svelte";
  import Icon from "./Icon.svelte";

  // Gimbal speed only scales joystick moves; OBSBOT Center sends nothing to
  // the camera for it. Remembered per machine.
  const SPEEDS = [
    { value: 0, label: "Slow" },
    { value: 1, label: "Med." },
    { value: 2, label: "Fast" },
  ];
  // Absolute-nudge step per tick (generic cameras) and velocity fraction
  // (cameras with a gimbal velocity command), per speed setting.
  const SPEED_STEP = [0.008, 0.02, 0.04];
  const SPEED_VELOCITY = [0.3, 0.6, 1.0];
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

  // Presets live on the camera: position, zoom and a name per slot. Empty
  // slots offer "Add"; filled ones recall on click, and can be overwritten
  // with the current view or renamed.
  const NAME_MAX = 16;
  let renaming = $state<number | null>(null);
  let draft = $state("");
  const slots = $derived(device.presets.length ? device.presets : [null, null, null]);

  function startRename(slot: number) {
    renaming = slot;
    draft = device.presets[slot] ?? "";
  }

  function finishRename() {
    if (renaming === null) return;
    const slot = renaming;
    renaming = null;
    const name = draft.trim();
    if (name && name !== device.presets[slot]) void device.renamePreset(slot, name);
  }

  function focus(el: HTMLInputElement) {
    el.select();
  }

  const IMAGE = ["brightness", "contrast", "saturation", "sharpness", "hue"];

  /** Show a dependent control when its parent is in `state`, or when the
   * parent is unsupported (so the full layout is still visible). */
  function when(parent: string, state: boolean): boolean {
    return !device.supported(parent) || device.on(parent) === state;
  }

  const gimbalSupported = $derived(
    device.gimbalVelocity || device.supported("pan") || device.supported("tilt"),
  );

  function joystick(x: number, y: number) {
    if (device.gimbalVelocity) {
      // Mirrored video: pushing right should move the picture right.
      const right = device.on("mirror_image") ? -x : x;
      const k = SPEED_VELOCITY[speed];
      void device.gimbalMove(right * k, y * k);
    } else {
      device.nudge("pan", x * SPEED_STEP[speed]);
      device.nudge("tilt", y * SPEED_STEP[speed]);
    }
  }

  function joystickRelease() {
    if (!device.gimbalVelocity) return;
    // Stop twice; a single lost stop would leave the gimbal drifting.
    void device.gimbalMove(0, 0).then(() => device.gimbalMove(0, 0));
  }
</script>

<div class="tab">
  <Card title="Preset">
    <div class="presets">
      {#each slots as name, slot (slot)}
        {#if !device.presets.length}
          <button disabled title="This camera has no presets">Add</button>
        {:else if renaming === slot}
          <input
            class="rename"
            maxlength={NAME_MAX}
            bind:value={draft}
            use:focus
            onblur={finishRename}
            onkeydown={(e) => {
              if (e.key === "Enter") finishRename();
              if (e.key === "Escape") renaming = null;
            }}
          />
        {:else if name === null}
          <button title="Save the current view as preset {slot + 1}" onclick={() => device.savePreset(slot, `Preset${slot + 1}`)}>
            Add
          </button>
        {:else}
          <div class="preset">
            <button class="recall" title="Go to {name}" onclick={() => device.recallPreset(slot)}>{name}</button>
            <button class="ghost icon" title="Save the current view to {name}" onclick={() => device.savePreset(slot, name)}>
              <Icon name="target" size={14} />
            </button>
            <button class="ghost icon" title="Rename" onclick={() => startRename(slot)}>
              <Icon name="pencil" size={14} />
            </button>
          </div>
        {/if}
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
      <Joystick disabled={!gimbalSupported} onmove={joystick} onrelease={joystickRelease} />
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
  .preset {
    display: flex;
    align-items: center;
    min-width: 0;
    background: var(--control);
    border-radius: var(--radius-sm);
  }
  .recall {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: transparent;
    padding-right: 2px;
  }
  .icon {
    padding: 4px;
    display: grid;
    place-items: center;
  }
  .rename {
    min-width: 0;
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
  .gimbal-speed :global(.segmented) {
    display: flex;
    width: 100%;
  }
  .gimbal-speed :global(.segmented button) {
    flex: 1;
    min-width: 0;
    padding: 3px 0;
  }
  .label.dim {
    color: var(--text-muted);
  }
</style>
