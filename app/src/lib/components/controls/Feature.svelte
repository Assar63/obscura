<script lang="ts">
  // Renders one catalog feature as a labelled row, choosing the control from
  // the feature's kind. Unsupported features render disabled with the reason
  // as a tooltip, so the layout matches OBSBOT Center even before the vendor
  // protocol is implemented.
  import { device } from "../../device.svelte";
  import Toggle from "../ui/Toggle.svelte";
  import Segmented from "../ui/Segmented.svelte";
  import SliderNumber from "../ui/SliderNumber.svelte";

  let {
    id,
    label,
    description,
    variant,
    hideUnsupported = false,
    stacked = false,
  }: {
    id: string;
    label?: string;
    description?: string;
    variant?: "segmented" | "select";
    hideUnsupported?: boolean;
    /** Put the label above the control (for narrow spots). */
    stacked?: boolean;
  } = $props();

  const f = $derived(device.features[id]);
  const disabled = $derived(!f?.supported || !f?.active);
  const tip = $derived(
    !f
      ? "unknown feature"
      : !f.supported
        ? (f.reason ?? "unsupported")
        : !f.active
          ? "inactive (controlled automatically)"
          : f.value === null
            ? "The camera doesn't report this setting; shown as off until changed here"
            : undefined,
  );
  const useSelect = $derived(
    f?.kind.type === "choice" && (variant === "select" || (!variant && f.kind.options.length > 3)),
  );
</script>

{#if f && (f.supported || !hideUnsupported)}
  <div class="row" class:ranged={f.kind.type === "range"} class:stacked title={tip}>
    <div class="text">
      <span class="label" class:dim={disabled}>{label ?? f.label}</span>
      {#if description}<span class="desc">{description}</span>{/if}
    </div>
    <div class="control">
      {#if f.kind.type === "toggle"}
        <Toggle checked={(f.value ?? 0) !== 0} {disabled} onchange={(v) => device.set(id, v ? 1 : 0)} />
      {:else if f.kind.type === "choice" && useSelect}
        <select
          value={f.value ?? ""}
          {disabled}
          onchange={(e) => device.set(id, Number((e.currentTarget as HTMLSelectElement).value))}
        >
          {#each f.kind.options as o (o.value)}
            <option value={o.value}>{o.label}</option>
          {/each}
        </select>
      {:else if f.kind.type === "choice"}
        <Segmented options={f.kind.options} value={f.value} {disabled} onchange={(v) => device.set(id, v)} />
      {:else if f.kind.type === "range"}
        <SliderNumber
          value={f.value}
          min={f.kind.min}
          max={f.kind.max}
          step={f.kind.step}
          scale={f.kind.scale}
          unit={f.kind.unit}
          {disabled}
          onchange={(v) => device.set(id, v)}
        />
      {:else}
        <button {disabled} onclick={() => device.set(id, 1)}>{label ?? f.label}</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 26px;
  }
  .row.ranged .text {
    width: 96px;
    flex-shrink: 0;
  }
  .row.ranged .control {
    flex: 1;
  }
  .row.stacked {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .row.stacked .text {
    width: auto;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  /* Toggles, choices and buttons keep their size; long descriptions wrap
     beside them instead of running underneath. */
  .row:not(.ranged) .control {
    flex-shrink: 0;
  }
  .label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .label.dim {
    color: var(--text-muted);
  }
  .desc {
    font-size: 12px;
    color: var(--text-muted);
  }
  .control {
    display: flex;
    justify-content: flex-end;
    min-width: 0;
  }
  select {
    min-width: 110px;
  }
</style>
