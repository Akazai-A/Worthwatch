<script lang="ts">
  // The Undo bar: shows after a removal that saved at once, for 8 seconds.
  import { undo } from "../../stores/undo.svelte";
</script>

{#if undo.current || undo.error}
  <div class="undo" role="status">
    {#if undo.current}
      <span>{undo.current.label}</span>
      <button type="button" class="act" onclick={() => undo.run()}>Undo</button>
      <button type="button" class="x" aria-label="Dismiss" onclick={() => undo.dismiss()}>×</button>
    {:else}<span class="err">Could not undo: {undo.error}</span>{/if}
  </div>
{/if}

<style>
  .undo { position: fixed; left: 50%; bottom: 24px; transform: translateX(-50%); z-index: 50; display: flex; align-items: center; gap: 16px; background: var(--surface-2); border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px 8px 14px; font-size: 13px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35); }
  .act { background: none; border: 0; color: var(--accent); font: inherit; font-weight: 600; cursor: pointer; padding: 2px 4px; }
  .x { background: none; border: 0; color: var(--text-3); font: inherit; font-size: 15px; cursor: pointer; padding: 0 4px; }
  .act:focus-visible, .x:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; border-radius: 3px; }
  .err { color: var(--negative); }
  @media (prefers-reduced-motion: no-preference) {
    .undo { animation: rise 220ms var(--ease-out); }
  }
  @keyframes rise { from { transform: translate(-50%, 12px); opacity: 0; } }
  @media (max-width: 760px) { .undo { bottom: calc(64px + env(safe-area-inset-bottom)); } }
</style>
