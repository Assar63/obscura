<script lang="ts">
  import { onMount } from "svelte";
  import { device } from "./lib/device.svelte";
  import { api } from "./lib/api";
  import TopBar from "./lib/components/layout/TopBar.svelte";
  import PreviewArea from "./lib/components/layout/PreviewArea.svelte";
  import BottomBar from "./lib/components/layout/BottomBar.svelte";
  import Sidebar from "./lib/components/layout/Sidebar.svelte";

  let preview = $state(true);

  onMount(() => {
    void device.init();
  });

  // Ctrl+Shift+D opens or closes the Diagnostics window.
  function onkeydown(e: KeyboardEvent) {
    if (e.ctrlKey && e.shiftKey && !e.altKey && e.key.toLowerCase() === "d") {
      e.preventDefault();
      api.toggleDiagnostics().catch((err) => (device.error = String(err)));
    }
  }
</script>

<svelte:window {onkeydown} />

<main>
  <div class="stage">
    <TopBar bind:preview />
    <PreviewArea {preview} />
    <BottomBar />
  </div>
  <Sidebar />
</main>

<style>
  main {
    display: flex;
    gap: 8px;
    height: 100%;
    padding: 8px;
    background: var(--bg);
  }
  .stage {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--surface);
    border-radius: var(--radius);
    overflow: hidden;
  }
</style>
