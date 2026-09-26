<script lang="ts">
  let {
    open = $bindable(false),
    title = "Tips",
    message,
    onconfirm,
    oncancel,
  }: {
    open?: boolean;
    title?: string;
    message: string;
    onconfirm: () => void;
    oncancel?: () => void;
  } = $props();

  function close(confirmed: boolean) {
    open = false;
    if (confirmed) onconfirm();
    else oncancel?.();
  }
</script>

{#if open}
  <div class="backdrop" role="presentation" onclick={() => close(false)}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && close(false)}
    >
      <h3>{title}</h3>
      <p>{message}</p>
      <div class="actions">
        <button onclick={() => close(false)}>No</button>
        <button class="primary" onclick={() => close(true)}>OK</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 100;
  }
  .dialog {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 20px 28px;
    min-width: 320px;
    max-width: 420px;
    text-align: center;
  }
  h3 {
    margin: 0 0 12px;
    font-weight: 500;
    font-size: 15px;
  }
  p {
    margin: 0 0 24px;
  }
  .actions {
    display: flex;
    gap: 32px;
    justify-content: center;
  }
  .actions button {
    min-width: 96px;
  }
</style>
