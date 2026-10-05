<script lang="ts">
  // Shown when sharing is switched on but the virtual camera (v4l2loopback)
  // isn't loaded: loading a kernel module needs root, so the user runs the
  // commands themselves.
  let {
    open = $bindable(false),
    commands,
    onrecheck,
  }: {
    open?: boolean;
    commands: string[];
    onrecheck: () => Promise<void>;
  } = $props();

  let copied = $state<number | null>(null);

  async function copy(i: number) {
    try {
      await navigator.clipboard.writeText(commands[i]);
      copied = i;
      setTimeout(() => (copied = copied === i ? null : copied), 1500);
    } catch {
      // Clipboard not available; the text can still be selected.
    }
  }

  async function recheck() {
    await onrecheck();
    open = false;
  }
</script>

{#if open}
  <div class="backdrop" role="presentation" onclick={() => (open = false)}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="share-setup-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && (open = false)}
    >
      <h3 id="share-setup-title">Set up the virtual camera</h3>
      <p>
        Sharing passes the camera to a virtual camera, <b>OBSCura Camera</b>, that OBS, Teams or a
        browser can use while OBSCura shows the preview. It's created by the <code>v4l2loopback</code>
        kernel module, which needs administrator rights to load, so run this in a terminal:
      </p>
      <div class="cmd">
        <code>{commands[0]}</code>
        <button class="ghost" onclick={() => copy(0)}>{copied === 0 ? "Copied" : "Copy"}</button>
      </div>
      <p class="muted">That lasts until the next restart. To load it at every start as well:</p>
      {#each commands.slice(1) as c, i (c)}
        <div class="cmd">
          <code>{c}</code>
          <button class="ghost" onclick={() => copy(i + 1)}>{copied === i + 1 ? "Copied" : "Copy"}</button>
        </div>
      {/each}
      <p class="muted">
        If <code>v4l2loopback</code> isn't installed: <code>sudo apt install v4l2loopback-dkms</code>.
        <code>tools/bootstrap.sh virtual-camera</code> does all of this too.
      </p>
      <div class="actions">
        <button onclick={() => (open = false)}>Close</button>
        <button class="primary" onclick={recheck}>Check again</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .dialog {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius);
    padding: 18px 20px;
    width: min(640px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow: auto;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
  h3 {
    margin: 0 0 8px;
  }
  p {
    margin: 8px 0;
    line-height: 1.4;
  }
  .cmd {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 6px 0;
  }
  .cmd code {
    flex: 1;
    display: block;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--control);
    font-size: 12px;
    word-break: break-all;
    user-select: all;
  }
  .muted {
    color: var(--text-muted);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
