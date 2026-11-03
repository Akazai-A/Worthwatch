<script lang="ts">
  // Portfolio › Exposure: sector, region, country, asset class and currency bars,
  // through funds or as held. One neutral fill; the labels carry identity.
  import type { Bucket, LookThroughView } from "../lib/api";
  import { formatDate, formatMoney } from "../lib/money";
  import { compactMoney, topBuckets } from "../lib/lookthrough";
  import Money from "../lib/components/Money.svelte";

  let { lt }: { lt: LookThroughView } = $props();
  const cur = $derived(lt.base_currency);
  let mode = $state<"lookthrough" | "as_held">("lookthrough");
  const x = $derived(mode === "lookthrough" ? lt.lookthrough : lt.as_held);
  const anyResolved = $derived(lt.lookthrough.funds.some((f) => f.resolved));
  type Set = { title: string; sub?: string; rows: Bucket[] };
  const sets = $derived<Set[]>([
    { title: "Sector", rows: x.by_sector },
    { title: "Region", rows: x.by_region },
    { title: "Country", sub: x.by_country.length > 8 ? "top 8" : undefined, rows: topBuckets(x.by_country, 8) },
    { title: "Asset class", rows: x.by_asset_class },
    { title: "Currency", sub: mode === "lookthrough" ? "of the underlying" : "traded in", rows: topBuckets(x.by_currency, 6) },
  ]);
  const muted = (label: string) => label === "Undisclosed" || label === "Unknown" || label === "Unclassified" || label.endsWith("others");
</script>

<div class="top">
  <div>
    <div class="label">Exposure</div>
    <div class="muted lead"><Money value={x.total} currency={cur} /> across {lt.holdings.length} {lt.holdings.length === 1 ? "holding" : "holdings"}{#if x.as_of}&nbsp;· holdings as of {formatDate(x.as_of)}{/if}</div>
  </div>
  <div class="toggle" role="radiogroup" aria-label="View">
    <button type="button" role="radio" aria-checked={mode === "lookthrough"} class:on={mode === "lookthrough"} onclick={() => (mode = "lookthrough")} disabled={!anyResolved} title={anyResolved ? "" : "No holdings files yet"}>Look-through</button>
    <button type="button" role="radio" aria-checked={mode === "as_held"} class:on={mode === "as_held"} onclick={() => (mode = "as_held")}>As held</button>
  </div>
</div>

{#if lt.holdings.length === 0}
  <section class="card empty"><p class="muted">Nothing to break down yet: add positions on <a href="#/check-in">Check-in</a>.</p></section>
{:else}
  <div class="bars">
    {#each sets as s (s.title)}
      {@const max = Math.max(...s.rows.map((r) => Number(r.pct)), 0.01)}
      <section class="card barset">
        <h2>{s.title}{#if s.sub}<small>{s.sub}</small>{/if}</h2>
        {#each s.rows as r (r.label)}
          <div class="row" class:faint={muted(r.label)} title="{r.label}: {formatMoney(r.value, cur)} ({r.pct}%)">
            <span class="name">{r.label}</span>
            <span class="track"><i style="width:{(Number(r.pct) / max) * 100}%" class:soft={muted(r.label)}></i></span>
            <span class="pct num">{Number(r.pct).toFixed(1)}%</span>
            <span class="amt money">{compactMoney(r.value)}</span>
          </div>
        {/each}
      </section>
    {/each}
  </div>
  <p class="faint note">
    {#if mode === "lookthrough"}Look-through follows each fund into what it owns, up to three levels, and sums the pieces. "Undisclosed" is the part of a fund its file does not list.{:else}As held treats each position as one line: a fund counts as a fund, in the currency and country it trades in.{/if}
  </p>
{/if}

<style>
  .top { display: flex; justify-content: space-between; align-items: flex-end; gap: 12px; flex-wrap: wrap; }
  .lead { font-size: 13px; margin-top: 2px; }
  .toggle { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
  .toggle button { padding: 5px 11px; font: inherit; font-size: 12px; color: var(--text-2); background: none; border: 0; cursor: pointer; }
  .toggle button.on { background: var(--accent-soft); color: var(--text); }
  .toggle button:disabled { opacity: 0.5; cursor: default; }
  .toggle button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .empty p { margin: 0; }
  .bars { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; }
  .barset h2 { margin: 0 0 10px; font-size: 12px; font-weight: 500; color: var(--text-3); display: flex; justify-content: space-between; }
  .barset h2 small { text-transform: none; letter-spacing: 0; font-size: 11px; }
  .row { display: grid; grid-template-columns: minmax(96px, 128px) 1fr 48px 60px; gap: 10px; align-items: center; font-size: 12px; padding: 3px 0; }
  .row .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .track { height: 10px; background: var(--surface-2); border-radius: 3px; overflow: hidden; position: relative; }
  .track i { position: absolute; left: 0; top: 0; bottom: 0; border-radius: 0 3px 3px 0; min-width: 1px; background: var(--fill); }
  .track i.soft { background: var(--fill-soft); }
  .pct { text-align: right; }
  .amt { text-align: right; color: var(--text-2); }
  .note { font-size: 12px; margin: 0; }
  @media (max-width: 760px) { .bars { grid-template-columns: 1fr; } .row { grid-template-columns: 96px 1fr 44px 52px; } }
</style>
