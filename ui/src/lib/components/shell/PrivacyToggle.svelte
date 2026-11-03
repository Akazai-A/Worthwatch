<script lang="ts">
  // The eye under each big figure: shows or hides every amount (same as ⌘⇧H).
  import { prefs } from "../../stores/prefs.svelte";
</script>

<button
  type="button"
  class="eye"
  aria-pressed={prefs.blur}
  aria-label={prefs.blur ? "Show amounts" : "Hide amounts"}
  title="{prefs.blur ? 'Show' : 'Hide'} amounts (⌘⇧H)"
  onclick={() => prefs.toggleBlur()}>
  <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
    <path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12Z" />
    <circle cx="12" cy="12" r="3" />
    {#if prefs.blur}<line x1="4" y1="20" x2="20" y2="4" />{/if}
  </svg>
</button>

<style>
  .eye { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 22px; margin-left: -6px; padding: 0; border: 0; border-radius: 4px; background: none; color: var(--text-2); cursor: pointer; vertical-align: middle; }
  .eye:hover { color: var(--text); background: var(--surface-2); }
  .eye:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .eye[aria-pressed="true"] { color: var(--text); }
  svg { fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
</style>
