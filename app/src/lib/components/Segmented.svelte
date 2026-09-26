<script lang="ts">
  import type { ChoiceOption } from "../api";

  let {
    options,
    value,
    disabled = false,
    onchange,
  }: {
    options: ChoiceOption[];
    value: number | null;
    disabled?: boolean;
    onchange: (v: number) => void;
  } = $props();
</script>

<div class="segmented">
  {#each options as o (o.value)}
    <button class:selected={o.value === value} {disabled} onclick={() => onchange(o.value)}>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    background: var(--bg);
    border-radius: var(--radius-sm);
    padding: 2px;
    gap: 2px;
  }
  button {
    background: transparent;
    padding: 3px 12px;
    font-size: 13px;
    color: var(--text-muted);
  }
  button.selected {
    background: var(--accent);
    color: #fff;
  }
  button.selected:hover:not(:disabled) {
    background: var(--accent-hover);
  }
</style>
