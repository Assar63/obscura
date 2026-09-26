<script lang="ts">
  import { onMount } from "svelte";
  import { device } from "./lib/device.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import PreviewArea from "./lib/components/PreviewArea.svelte";
  import BottomBar from "./lib/components/BottomBar.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";

  let preview = $state(true);

  onMount(() => {
    void device.init();
  });
</script>

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
