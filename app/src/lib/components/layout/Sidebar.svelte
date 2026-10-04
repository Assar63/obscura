<script lang="ts">
  import ImageTab from "../tabs/ImageTab.svelte";
  import AudioTab from "../tabs/AudioTab.svelte";
  import MoreTab from "../tabs/MoreTab.svelte";

  let tab = $state<"image" | "audio" | "more">("image");
</script>

<aside class="sidebar">
  <nav>
    <button class:sel={tab === "image"} onclick={() => (tab = "image")}>Image</button>
    <button class:sel={tab === "audio"} onclick={() => (tab = "audio")}>Audio</button>
    <button class:sel={tab === "more"} onclick={() => (tab = "more")}>More</button>
  </nav>
  <div class="scroll">
    {#if tab === "image"}<ImageTab />{:else if tab === "audio"}<AudioTab />{:else}<MoreTab />{/if}
  </div>
</aside>

<style>
  .sidebar {
    width: 320px;
    flex-shrink: 0;
    background: var(--panel);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  nav {
    display: flex;
    justify-content: space-around;
    padding: 8px 8px 4px;
  }
  nav button {
    background: transparent;
    color: var(--text-muted);
    font-size: 15px;
    position: relative;
    padding: 8px 16px;
  }
  nav button.sel {
    color: var(--text);
  }
  nav button.sel::after {
    content: "";
    position: absolute;
    left: 50%;
    bottom: 0;
    width: 18px;
    height: 2px;
    margin-left: -9px;
    background: var(--accent);
    border-radius: 1px;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 8px 12px 12px;
    min-height: 0;
  }
</style>
