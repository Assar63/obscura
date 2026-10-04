<script lang="ts">
  // Anchored popover that closes on outside click or Escape.
  import type { Snippet } from "svelte";

  let {
    open = $bindable(false),
    placement = "below",
    trigger,
    children,
  }: {
    open?: boolean;
    placement?: "below" | "above";
    trigger: Snippet;
    children: Snippet;
  } = $props();

  let root: HTMLElement;

  function onWindowClick(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) open = false;
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === "Escape" && (open = false)} />

<div class="popover-root" bind:this={root}>
  {@render trigger()}
  {#if open}
    <div class="popover {placement}">{@render children()}</div>
  {/if}
</div>

<style>
  .popover-root {
    position: relative;
    display: inline-flex;
  }
  .popover {
    position: absolute;
    left: 0;
    z-index: 50;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px 14px;
    min-width: 280px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }
  .below {
    top: calc(100% + 8px);
  }
  .above {
    bottom: calc(100% + 8px);
  }
</style>
