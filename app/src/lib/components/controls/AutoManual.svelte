<script lang="ts">
  // One setting with an automatic and a manual mode (e.g. white balance):
  // an Auto | Manual switch on the auto toggle feature, with the manual
  // controls below it in Manual, or the auto-only ones in Auto. Without the
  // auto toggle, the manual controls are shown on their own.
  import { device } from "../../device.svelte";
  import Feature from "./Feature.svelte";
  import Segmented from "../ui/Segmented.svelte";

  let {
    label,
    autoId,
    manual,
    auto = [],
    autoNote,
  }: {
    label: string;
    /** Toggle feature that is on in Auto. */
    autoId: string;
    /** Features shown in Manual. */
    manual: string[];
    /** Features shown in Auto. */
    auto?: string[];
    /** Shown in Auto when there are no auto-only features. */
    autoNote?: string;
  } = $props();

  const MODES = [
    { value: 1, label: "Auto" },
    { value: 0, label: "Manual" },
  ];

  const f = $derived(device.features[autoId]);
  const supported = $derived(!!f?.supported);
  const isAuto = $derived(supported && device.on(autoId));
  const tip = $derived(supported ? undefined : (f?.reason ?? "unsupported"));
</script>

<div class="group">
  {#if supported || !manual.some((id) => device.supported(id))}
    <div class="row" title={tip}>
      <span class="label" class:dim={!supported}>{label}</span>
      <Segmented
        options={MODES}
        value={supported ? (isAuto ? 1 : 0) : null}
        disabled={!supported || !f?.active}
        onchange={(v) => device.set(autoId, v)}
      />
    </div>
  {/if}
  {#if isAuto}
    {#each auto as id (id)}<Feature {id} />{/each}
    {#if !auto.length && autoNote}<p class="note">{autoNote}</p>{/if}
  {:else}
    {#each manual as id (id)}<Feature {id} hideUnsupported={supported} />{/each}
  {/if}
</div>

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 26px;
  }
  .label {
    white-space: nowrap;
  }
  .label.dim {
    color: var(--text-muted);
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
