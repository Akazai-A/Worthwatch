<script lang="ts">
  // Chat docked on the right of whatever screen is open, so the numbers it changes stay in view.
  import type { LedgerView } from "../api";
  import { drawer } from "../stores/drawer.svelte";
  import Chat from "../../screens/Chat.svelte";
  let { view }: { view: LedgerView } = $props();
</script>

<aside class="drawer" aria-label="Ask or update">
  <div class="dh">
    <h2>Ask or update</h2>
    <span class="faint">nothing is saved until you approve</span>
    <button type="button" class="x" onclick={() => drawer.hide()} aria-label="Close (Esc)" title="Close (Esc)">×</button>
  </div>
  <Chat {view} docked />
</aside>

<style>
  .drawer { position: fixed; top: 0; right: 0; bottom: 0; width: 420px; z-index: 45; background: var(--bg); border-left: 1px solid var(--border); display: flex; flex-direction: column; padding: 16px 16px 12px; gap: 8px; }
  .dh { display: flex; align-items: baseline; gap: 10px; }
  .dh h2 { margin: 0; font-size: 13px; font-weight: 600; }
  .dh .faint { font-size: 12px; }
  .x { margin-left: auto; background: none; border: 0; color: var(--text-2); font-size: 20px; line-height: 1; cursor: pointer; padding: 0 4px; }
  .x:hover { color: var(--text); }
  .x:focus-visible { outline: 2px solid var(--accent); }
  @media (max-width: 760px) { .drawer { width: auto; left: 0; bottom: calc(53px + env(safe-area-inset-bottom)); border-left: 0; } }
</style>
