<script lang="ts">
  // Slider with a numeric box, like OBSBOT Center's "1.00 x" controls.
  // Values are raw integers; `scale` converts to display units.
  let {
    value,
    min,
    max,
    step = 1,
    scale = 1,
    unit = null,
    disabled = false,
    onchange,
  }: {
    value: number | null;
    min: number;
    max: number;
    step?: number;
    scale?: number;
    unit?: string | null;
    disabled?: boolean;
    onchange: (v: number) => void;
  } = $props();

  const decimals = $derived(scale > 1 ? Math.min(2, Math.ceil(Math.log10(scale))) : 0);
  const current = $derived(value ?? min);
  const pct = $derived(max > min ? ((current - min) / (max - min)) * 100 : 0);
  const display = $derived(value === null ? "–" : (current / scale).toFixed(decimals));

  function clamp(v: number) {
    return Math.min(max, Math.max(min, Math.round(v / step) * step));
  }

  function commitText(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const n = parseFloat(input.value);
    if (Number.isFinite(n)) onchange(clamp(Math.round(n * scale)));
    else input.value = display;
  }
</script>

<div class="slider-number" class:disabled>
  <input
    type="range"
    {min}
    {max}
    {step}
    value={current}
    {disabled}
    style="--pct: {pct}%"
    oninput={(e) => onchange(Number((e.currentTarget as HTMLInputElement).value))}
  />
  <div class="number">
    <input
      type="text"
      inputmode="decimal"
      value={display}
      {disabled}
      onchange={commitText}
      onkeydown={(e) => {
        if (e.key === "ArrowUp") onchange(clamp(current + step));
        if (e.key === "ArrowDown") onchange(clamp(current - step));
      }}
    />
    {#if unit}<span class="unit">{unit}</span>{/if}
  </div>
</div>

<style>
  .slider-number {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
  }
  .disabled {
    opacity: 0.45;
  }
  input[type="range"] {
    flex: 1;
    min-width: 60px;
    appearance: none;
    height: 3px;
    padding: 0;
    border: 0;
    border-radius: 2px;
    background: linear-gradient(
      to right,
      var(--accent) 0 var(--pct),
      var(--control) var(--pct) 100%
    );
  }
  input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #fff;
    cursor: pointer;
  }
  .number {
    display: flex;
    align-items: center;
    background: var(--bg);
    border-radius: var(--radius-sm);
    width: 72px;
    padding-right: 6px;
  }
  .number input {
    width: 100%;
    background: transparent;
    text-align: right;
    padding: 3px 4px;
  }
  .unit {
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
