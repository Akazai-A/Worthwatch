<script lang="ts">
  // "prices as of Oct 31 · USD/CAD 1.3912 (Bank of Canada, Oct 31) · Refresh"
  import { refresh, type LedgerView } from "../../api";
  import { errorSummary, priceStatus, rateLabels } from "../../portfolio/market";
  import { view as viewStore } from "../../stores/view.svelte";

  let { view }: { view: LedgerView } = $props();
  let busy = $state(false);
  const status = $derived(priceStatus(view.market));
  const rates = $derived(rateLabels(view.market, view.base_currency));
  const errors = $derived(errorSummary(view.market));
  const canFetch = $derived(!view.market.offline && (view.market.price_provider !== "off" || view.market.fx_provider !== "off"));

  async function go() {
    if (busy) return;
    busy = true;
    try {
      viewStore.set(await refresh());
    } finally {
      busy = false;
    }
  }
</script>

<span class="line">
  <span class={status.tone} title={view.market.fetched_at ? `last fetch ${new Date(view.market.fetched_at).toLocaleString()}` : undefined}>{status.text}</span>
  {#each rates as r}<span class="sep">·</span><span class={r.tone}>{r.text}</span>{/each}
  {#if errors}<span class="sep">·</span><span class="warn" title={Object.entries(view.market.errors).map(([k, v]) => `${k}: ${v}`).join("\n")}>{errors}</span>{/if}
  {#if canFetch}
    <button type="button" class="ghost" id="refresh" onclick={go} disabled={busy}>{busy ? "Fetching…" : "Refresh"}</button>
  {/if}
</span>

<style>
  .line { display: inline-flex; align-items: center; gap: 6px; flex-wrap: wrap; font-size: 12px; color: var(--text-3); }
  .ok { color: var(--text-2); }
  .warn { color: var(--warn); }
  .faint { color: var(--text-3); }
  .sep { color: var(--text-3); }
  .ghost { margin-left: 6px; background: none; border: 1px solid var(--border); color: var(--text-2); border-radius: 6px; padding: 3px 9px; font: inherit; font-size: 12px; cursor: pointer; }
  .ghost:hover { color: var(--text); border-color: var(--text-3); }
  .ghost:disabled { opacity: 0.6; cursor: default; }
  .ghost:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
