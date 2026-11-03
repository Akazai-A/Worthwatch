<script lang="ts">
  // Portfolio › Exposure: one readout per question (country, asset class, currency,
  // sector): the biggest share as a number, one bar with the next three, and the full
  // list on demand. Region is left out: it repeats country. Through funds or as held.
  import type { Bucket, LookThroughView } from "../../lib/api";
  import { formatDate, formatMoney, formatPct } from "../../lib/format/money";
  import { compactMoney } from "../../lib/portfolio/lookthrough";
  import Money from "../../lib/components/amounts/Money.svelte";

  let { lt }: { lt: LookThroughView } = $props();
  const cur = $derived(lt.base_currency);
  let mode = $state<"lookthrough" | "as_held">("lookthrough");
  const x = $derived(mode === "lookthrough" ? lt.lookthrough : lt.as_held);
  const anyResolved = $derived(lt.lookthrough.funds.some((f) => f.resolved));
  type Set = { title: string; sub?: string; rows: Bucket[] };
  const sets = $derived<Set[]>([
    { title: "Country", rows: x.by_country },
    { title: "Asset class", rows: x.by_asset_class },
    { title: mode === "lookthrough" ? "Currency of the underlying" : "Currency traded in", rows: x.by_currency },
    { title: "Sector", rows: x.by_sector },
  ]);
  let open = $state<Record<string, boolean>>({});
  const muted = (label: string) => label === "Undisclosed" || label === "Unknown" || label === "Unclassified" || label.endsWith("others");
</script>

<div class="top">
  <div>
    <h1 class="label">Exposure</h1>
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
  <div class="reads">
    {#each sets as s (s.title)}
      {@const top = s.rows[0]}
      {@const next = s.rows.slice(1, 4)}
      {#if top}
        <section class="read">
          <div class="big">
            <div class="faint t">{s.title}</div>
            <div class="pc">{Math.round(Number(top.pct))}%</div>
            <div class="nm">{top.label}</div>
          </div>
          <div class="rest">
            <div class="one" title="{s.rows.map((r) => `${r.label} ${formatPct(r.pct)}`).join(' · ')}">
              <i style="width:{top.pct}%"></i>
              {#each next as r, i (r.label)}<i class="n" style="width:{r.pct}%;opacity:{0.7 - i * 0.18}"></i>{/each}
            </div>
            <div class="nx">
              {#each next as r (r.label)}<span>{r.label} <b class="num">{formatPct(r.pct)}</b></span>{/each}
              {#if s.rows.length > 1}<button type="button" class="all" aria-expanded={!!open[s.title]} onclick={() => (open[s.title] = !open[s.title])}>{open[s.title] ? "Hide" : `All ${s.rows.length}`}</button>{/if}
            </div>
            {#if open[s.title]}
              {@const max = Math.max(...s.rows.map((r) => Number(r.pct)), 0.01)}
              <div class="list">
                {#each s.rows as r (r.label)}
                  <div class="row" class:faint={muted(r.label)} title="{r.label}: {formatMoney(r.value, cur)} ({r.pct}%)">
                    <span class="name">{r.label}</span>
                    <span class="track"><i style="width:{(Number(r.pct) / max) * 100}%" class:soft={muted(r.label)}></i></span>
                    <span class="pct num">{formatPct(r.pct)}</span>
                    <span class="amt money">{compactMoney(r.value)}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </section>
      {/if}
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
  .reads { display: flex; flex-direction: column; }
  .read { display: grid; grid-template-columns: 200px 1fr; gap: 24px; padding: 14px 0; border-top: 1px solid var(--border); }
  .t { font-size: 12px; }
  .pc { font-family: var(--display); font-size: 2.25rem; line-height: 1.1; font-variant-numeric: lining-nums; }
  .nm { font-size: 14px; }
  .rest { min-width: 0; padding-top: 22px; }
  .one { display: flex; gap: 2px; height: 14px; border-radius: 3px; overflow: hidden; background: var(--surface-2); }
  .one i { display: block; height: 100%; background: var(--fill); }
  .nx { display: flex; flex-wrap: wrap; gap: 4px 16px; margin-top: 8px; font-size: 12px; color: var(--text-2); align-items: baseline; }
  .nx b { color: var(--text); font-weight: 500; }
  .all { margin-left: auto; background: none; border: 0; padding: 0; font: inherit; font-size: 12px; color: var(--text-2); cursor: pointer; text-decoration: underline; text-underline-offset: 3px; }
  .all:hover { color: var(--text); }
  .all:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .list { margin-top: 10px; }
  .row { display: grid; grid-template-columns: minmax(96px, 128px) 1fr 48px 60px; gap: 10px; align-items: center; font-size: 12px; padding: 3px 0; }
  .row .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .track { height: 10px; background: var(--surface-2); border-radius: 3px; overflow: hidden; position: relative; }
  .track i { position: absolute; left: 0; top: 0; bottom: 0; border-radius: 0 3px 3px 0; min-width: 1px; background: var(--fill); }
  .track i.soft { background: var(--fill-soft); }
  .pct { text-align: right; }
  .amt { text-align: right; color: var(--text-2); }
  .note { font-size: 12px; margin: 0; }
  @media (max-width: 760px) { .read { grid-template-columns: 1fr; gap: 6px; } .rest { padding-top: 0; } .row { grid-template-columns: 96px 1fr 44px 52px; } }
</style>
