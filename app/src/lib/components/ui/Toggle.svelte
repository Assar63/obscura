<script lang="ts">
  let {
    checked,
    disabled = false,
    onchange,
  }: { checked: boolean; disabled?: boolean; onchange: (v: boolean) => void } = $props();
</script>

<button
  class="toggle"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  {disabled}
  onclick={() => onchange(!checked)}
>
  <span class="label">{checked ? "ON" : "OFF"}</span>
  <span class="knob"></span>
</button>

<style>
  .toggle {
    position: relative;
    width: 42px;
    height: 22px;
    padding: 0;
    border-radius: 11px;
    background: var(--control);
    flex-shrink: 0;
  }
  .toggle.on {
    background: var(--accent);
  }
  .toggle.on:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #8f9097;
    transition: left 0.15s;
  }
  .on .knob {
    left: 23px;
    background: #fff;
  }
  .label {
    position: absolute;
    top: 5px;
    font-size: 9px;
    font-weight: 600;
    right: 5px;
    color: var(--text-muted);
  }
  .on .label {
    left: 6px;
    right: auto;
    color: #fff;
  }
</style>
