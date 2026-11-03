<script lang="ts">
  // "holdings as of Oct 30 · 8 funds on file · Update holdings", with per-fund errors in brass.
  import { refreshHoldings, type LookThroughView } from "../../api";
  import { formatDate } from "../../format/money";
  import { lookthrough as store } from "../../stores/portfolio/lookthrough.svelte";

  let { lt }: { lt: LookThroughView } = $props();
  let busy = $state(false);
  const asOf = $derived(lt.lookthrough.as_of);
  const errorCount = $derived(Object.keys(lt.errors).length);
  const sources = $derived([...new Set(lt.snapshots.map((s) => s.source))].filter((s) => s !== "seed"));

  async function go() {
    if (busy) return;
    busy = true;
    try {
      store.set(await refreshHoldings());
    } finally {
      busy = false;
    }
  }
</script>

<span class="line">
  {#if lt.snapshots.length === 0}
    <span class="warn">no holdings files yet</span>
  {:else}
    <span class="ok" title={lt.fetched_at ? `last fetch ${new Date(lt.fetched_at).toLocaleString()}` : undefined}>holdings as of {asOf ? formatDate(asOf) : "—"}{#if sources.length}&nbsp;({sources.join(", ")}){/if}</span>
    <span class="sep">·</span><span>{lt.snapshots.length} {lt.snapshots.length === 1 ? "fund" : "funds"} on file</span>
  {/if}
  {#if errorCount}
    <span class="sep">·</span>
    <span class="warn" title={Object.entries(lt.errors).map(([k, v]) => `${k}: ${v}`).join("\n")}>{errorCount} {errorCount === 1 ? "fund" : "funds"} could not be read</span>
  {/if}
  {#if lt.offline}<span class="sep">·</span><span>offline</span>{/if}
  <button type="button" class="ghost" id="refresh-holdings" onclick={go} disabled={busy} title={lt.offline ? "Re-read the holdings folder" : "Fetch issuer holdings files for every fund you hold"}>
    {busy ? "Working…" : lt.offline ? "Re-read folder" : "Update holdings"}
  </button>
</span>

<style>
  .line { display: inline-flex; align-items: center; gap: 6px; flex-wrap: wrap; font-size: 12px; color: var(--text-3); }
  .ok { color: var(--text-2); }
  .warn { color: var(--warn); }
  .sep { color: var(--text-3); }
  .ghost { margin-left: 6px; background: none; border: 1px solid var(--border); color: var(--text-2); border-radius: 6px; padding: 3px 9px; font: inherit; font-size: 12px; cursor: pointer; }
  .ghost:hover { color: var(--text); border-color: var(--text-3); }
  .ghost:disabled { opacity: 0.6; cursor: default; }
  .ghost:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
