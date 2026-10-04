<script lang="ts">
  // The camera's microphones. Wireless microphones (Vox SE) belong here too
  // once they're supported (see TODO.md).
  import Card from "./Card.svelte";
  import Feature from "./Feature.svelte";
</script>

<div class="tab">
  <Card title="Microphone">
    <Feature id="disable_microphone" />
    <Feature id="noise_reduction" variant="select" />
    <Feature id="auto_gain" />
    <Feature id="pickup_distance" variant="select" />
    <Feature id="mic_during_sleep" />
  </Card>
</div>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
</style>
