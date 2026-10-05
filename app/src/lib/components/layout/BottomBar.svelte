<script lang="ts">
  import { device } from "../../device.svelte";
  import Icon from "../ui/Icon.svelte";
  import Popover from "../ui/Popover.svelte";
  import ConfirmDialog from "../ui/ConfirmDialog.svelte";
  import { preview } from "../../preview.svelte";

  // OBSBOT Center's Portrait mode and rotate/flip buttons send nothing to
  // the camera: they transform its virtual camera output on the PC, so
  // they're not offered here.
  let formatOpen = $state(false);
  let confirmLock = $state(false);
  const aiMode = $derived(device.features.ai_mode?.value ?? 0);
  const aiSupported = $derived(device.supported("ai_mode"));
  const locked = $derived(device.on("ai_lock"));

  const modes = [
    { value: 2, label: "Human", icon: "person" },
    { value: 1, label: "Group", icon: "group" },
    { value: 3, label: "Hand Tracking", icon: "hand" },
  ];

  // Modes beyond these that the camera's profile offers (e.g. the Tiny 3's
  // Whiteboard, Desk and Voice Tracking), shown in a second column.
  const extraIcons: Record<number, string> = { 4: "whiteboard", 5: "desk", 7: "mic" };
  const extraModes = $derived.by(() => {
    const kind = device.features.ai_mode?.kind;
    if (kind?.type !== "choice") return [];
    return kind.options
      .filter((o) => o.value !== 0 && !modes.some((m) => m.value === o.value))
      .map((o) => ({ value: o.value, label: o.label, icon: extraIcons[o.value] ?? "target" }));
  });

  function selectMode(value: number) {
    device.set("ai_mode", aiMode === value ? 0 : value);
  }
</script>

<footer class="bottombar">
  <div class="left">
    <Popover bind:open={formatOpen} placement="above">
      {#snippet trigger()}
        <button class="format" onclick={() => (formatOpen = !formatOpen)}>
          <span class="aspect">16:9</span>
          {preview.resolution}P {preview.fps}
          <Icon name="chevron-down" size={14} />
        </button>
      {/snippet}
      <div class="format-pop">
        <div class="grid">
          <button class:sel={preview.resolution === 1080} onclick={() => (preview.resolution = 1080)}>1080P (Full HD)</button>
          <button class:sel={preview.fps === 30} onclick={() => (preview.fps = 30)}>30 (Smooth)</button>
          <button class:sel={preview.resolution === 720} onclick={() => (preview.resolution = 720)}>720P (HD)</button>
          <button class:sel={preview.fps === 60} onclick={() => (preview.fps = 60)}>60 (Ultra Smooth)</button>
        </div>
      </div>
    </Popover>
    <button
      class="ghost icon mirror"
      class:sel={preview.mirror}
      aria-pressed={preview.mirror}
      title={preview.mirror ? "Show the preview unmirrored" : "Mirror the preview (only here; other apps still get the normal picture)"}
      onclick={() => preview.toggleMirror()}
    >
      <Icon name="mirror" size={16} />
    </button>
  </div>

  <div class="ai" title={aiSupported ? undefined : (device.features.ai_mode?.reason ?? "")}>
    {#if locked}
      <div class="locked">
        Please unlock first
        <button class="primary icon" title="Unlock AI" onclick={() => device.set("ai_lock", 0)}>
          <Icon name="lock" size={15} />
        </button>
      </div>
    {:else}
      <button
        class="mode human"
        class:sel={aiMode === modes[0].value}
        disabled={!aiSupported}
        onclick={() => selectMode(modes[0].value)}
      >
        <Icon name={modes[0].icon} size={20} />
        <span>{modes[0].label}</span>
      </button>
      <div class="stack">
        {#each modes.slice(1) as m (m.value)}
          <button class="mode small" class:sel={aiMode === m.value} disabled={!aiSupported} onclick={() => selectMode(m.value)}>
            <Icon name={m.icon} size={14} />
            <span>{m.label}</span>
          </button>
        {/each}
      </div>
      {#if extraModes.length}
        <div class="stack">
          {#each extraModes as m (m.value)}
            <button class="mode small" class:sel={aiMode === m.value} disabled={!aiSupported} onclick={() => selectMode(m.value)}>
              <Icon name={m.icon} size={14} />
              <span>{m.label}</span>
            </button>
          {/each}
        </div>
      {/if}
      <button
        class="ghost icon"
        title="Disable AI features and lock"
        disabled={!device.supported("ai_lock")}
        onclick={() => (confirmLock = true)}
      >
        <Icon name="unlock" size={16} />
      </button>
    {/if}
  </div>

  <div class="right">
    {#if device.current}
      <span class="status">{device.profileName}</span>
    {/if}
  </div>
</footer>

<ConfirmDialog bind:open={confirmLock} message="Disable AI features and lock?" onconfirm={() => device.set("ai_lock", 1)} />

<style>
  .bottombar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding: 6px 12px;
    min-height: 68px;
    gap: 12px;
  }
  .left {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .mirror.sel {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .right {
    display: flex;
    justify-content: flex-end;
  }
  .status {
    font-size: 12px;
    color: var(--text-muted);
  }
  .format {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--surface);
  }
  .aspect {
    border: 1px solid var(--text-muted);
    border-radius: 3px;
    font-size: 10px;
    padding: 0 3px;
  }
  .icon {
    padding: 5px;
    display: grid;
    place-items: center;
  }
  .format-pop {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 300px;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
  }
  .grid button {
    background: transparent;
  }
  .grid button.sel {
    background: var(--control);
  }
  .grid button.sel::before {
    content: "✓ ";
    color: var(--accent);
  }
  .ai {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--surface);
    border-radius: var(--radius-sm);
    padding: 4px;
  }
  .mode {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    font-size: 12px;
  }
  .mode.human {
    flex-direction: column;
    gap: 2px;
    width: 64px;
    height: 56px;
    justify-content: center;
  }
  .mode.small {
    padding: 3px 10px;
    width: 140px;
  }
  .mode.sel {
    background: var(--accent);
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .locked {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    font-size: 13px;
  }
</style>
