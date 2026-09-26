<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    open = $bindable(true),
    onreset,
    header,
    children,
  }: {
    title: string;
    open?: boolean;
    onreset?: () => void;
    header?: Snippet;
    children: Snippet;
  } = $props();
</script>

<section class="card">
  <div class="head">
    <span class="title">{title}</span>
    {#if onreset}
      <button class="ghost icon" title="Reset" onclick={onreset}><Icon name="reset" size={14} /></button>
    {/if}
    <span class="spacer"></span>
    {@render header?.()}
    <button class="ghost icon" aria-label={open ? "Collapse" : "Expand"} onclick={() => (open = !open)}>
      <Icon name={open ? "chevron-up" : "chevron-down"} />
    </button>
  </div>
  {#if open}
    <div class="body">{@render children()}</div>
  {/if}
</section>

<style>
  .card {
    background: var(--card);
    border-radius: var(--radius);
    padding: 12px 14px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .title {
    font-size: 14px;
  }
  .spacer {
    flex: 1;
  }
  .icon {
    padding: 3px;
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 14px;
  }
</style>
