<script lang="ts">
  // Circular pan/tilt pad. While held, reports the knob offset (-1..1 on
  // each axis) every `interval` ms; springs back on release.
  let {
    disabled = false,
    interval = 100,
    onmove,
  }: { disabled?: boolean; interval?: number; onmove: (x: number, y: number) => void } = $props();

  let pad: HTMLDivElement;
  let x = $state(0);
  let y = $state(0);
  let timer: ReturnType<typeof setInterval> | undefined;

  function update(e: PointerEvent) {
    const r = pad.getBoundingClientRect();
    let dx = (e.clientX - (r.left + r.width / 2)) / (r.width / 2 - 14);
    let dy = (e.clientY - (r.top + r.height / 2)) / (r.height / 2 - 14);
    const len = Math.hypot(dx, dy);
    if (len > 1) {
      dx /= len;
      dy /= len;
    }
    x = dx;
    y = -dy;
  }

  function down(e: PointerEvent) {
    if (disabled) return;
    pad.setPointerCapture(e.pointerId);
    update(e);
    timer = setInterval(() => onmove(x, y), interval);
  }

  function up() {
    clearInterval(timer);
    timer = undefined;
    x = 0;
    y = 0;
  }
</script>

<div
  class="pad"
  class:disabled
  bind:this={pad}
  role="slider"
  aria-label="Pan and tilt"
  aria-valuenow={0}
  tabindex="0"
  onpointerdown={down}
  onpointermove={(e) => timer && update(e)}
  onpointerup={up}
  onpointercancel={up}
>
  <span class="arrow n">▴</span><span class="arrow s">▾</span><span class="arrow w">◂</span><span class="arrow e">▸</span>
  <div class="ring">
    <div class="knob" style="transform: translate({x * 26}px, {-y * 26}px)"></div>
  </div>
</div>

<style>
  .pad {
    position: relative;
    width: 112px;
    height: 112px;
    flex-shrink: 0;
    border-radius: 50%;
    background: radial-gradient(circle, #34353b 0 55%, #2a2b30 56%);
    border: 1px solid var(--border);
    display: grid;
    place-items: center;
    touch-action: none;
    cursor: grab;
  }
  .pad.disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .ring {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
  .knob {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: #f2f2f4;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
  }
  .arrow {
    position: absolute;
    font-size: 9px;
    color: var(--text-dim);
  }
  .n { top: 4px; }
  .s { bottom: 4px; }
  .w { left: 6px; }
  .e { right: 6px; }
</style>
