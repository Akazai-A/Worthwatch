<script lang="ts">
  import PrivacyToggle from "../lib/components/PrivacyToggle.svelte";
  // Portfolio › Look-through: where the money actually is, through every fund.
  import type { Company, LookThroughView } from "../lib/api";
  import { formatDate, formatMoney } from "../lib/money";
  import { caveats, familyHue, familyOf, headlineParts, shortPath, viaLabel } from "../lib/lookthrough";
  import Money from "../lib/components/Money.svelte";
  import Treemap from "../lib/charts/Treemap.svelte";

  let { lt }: { lt: LookThroughView } = $props();
  const cur = $derived(lt.base_currency);
  const x = $derived(lt.lookthrough);
  // Ten rows sit level with the detail and funds cards whatever the company count;
  // the rest unfold on request, with the detail card pinned alongside.
  const SHOW = 10;
  const MAX = 50;
  const ranked = $derived(x.companies.filter((c) => !c.undisclosed && c.key !== "__other__").slice(0, MAX));
  let expanded = $state(false);
  const top = $derived(expanded ? ranked : ranked.slice(0, SHOW));
  const more = $derived(ranked.length - Math.min(ranked.length, SHOW));
  let selectedKey = $state<string | null>(null);
  const selected = $derived<Company | null>(x.companies.find((c) => c.key === selectedKey) ?? top[0] ?? null);
  const parts = $derived(headlineParts(x.company_count, x.funds, x.top10_pct, x.effective_holdings));
  const warns = $derived(caveats(x.undisclosed, x.total, x.funds));
  const maxTop = $derived(top.length ? Number(top[0].value) : 1);
  const unresolvedFunds = $derived(x.funds.filter((f) => !f.resolved));
  const broken = $derived(Object.entries(lt.errors));
  const missingChildren = $derived([...new Set(x.funds.flatMap((f) => f.unresolved_children))]);
  const dot = (c: Company) => `background:var(${familyHue(familyOf(c.slot))})`;
</script>

<section class="hero">
  <div class="label">Where the money actually is</div>
  <Money value={x.total} currency={cur} size="xl" />
  <div class="sub">
    <PrivacyToggle />
    {#each parts as p}<span class="chip">{p}</span>{/each}
    {#each warns as w}<span class="chip warn">{w}</span>{/each}
  </div>
</section>

{#if lt.snapshots.length === 0}
  <section class="card empty">
    <h2>No holdings files yet</h2>
    <p class="muted">
      Look-through needs each fund's holdings file. {#if lt.offline}Offline, so nothing is fetched:{:else}Press <em>Update holdings</em> above to fetch them from the issuer, or{/if}
      download the holdings CSV from the fund's page and save it as <code title={lt.holdings_dir}>{shortPath(lt.holdings_dir)}/&lt;SYMBOL&gt;.csv</code> (for example <code>XEQT.TO.csv</code>), then press <em>{lt.offline ? "Re-read folder" : "Update holdings"}</em>.
    </p>
    {#if broken.length}
      <ul class="errs">{#each broken as [k, v]}<li><b>{k}</b> <span class="warn">{v}</span></li>{/each}</ul>
    {/if}
  </section>
{:else}
  <section class="card chart">
    <Treemap companies={x.companies} currency={cur} selected={selected?.key ?? null} onselect={(k) => (selectedKey = k)} />
    <p class="legend">Sized by your dollars, grouped by sector. Click a cell to see which funds hold it.</p>
  </section>

  <div class="two">
    <section class="card">
      <h2 class="label">Top {top.length} companies</h2>
      <table class="top">
        <thead><tr><th>Company</th><th class="r">Value</th><th class="bar"></th><th class="r">Of total</th></tr></thead>
        <tbody>
          {#each top as c (c.key)}
            <tr class:on={selected?.key === c.key} onclick={() => (selectedKey = c.key)}>
              <td><i class="dot" style={dot(c)}></i>{c.name}<span class="sub2">{c.sector}</span></td>
              <td class="r"><Money value={c.value} currency={cur} /></td>
              <td class="bar"><span class="wt"><i style="width:{(Number(c.value) / maxTop) * 100}%"></i></span></td>
              <td class="r num">{Number(c.pct).toFixed(1)}%</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <div class="foot">
        {#if more > 0}
          <button type="button" class="more" onclick={() => (expanded = !expanded)}>{expanded ? "Show fewer" : `Show ${more} more`}</button>
        {/if}
        {#if x.company_count > ranked.length}
          <span class="faint">{(x.company_count - ranked.length).toLocaleString("en-CA")} more names, each under {Number(ranked[ranked.length - 1]?.pct ?? 0).toFixed(1)}%</span>
        {/if}
      </div>
    </section>

    <div class="side">
    <section class="card detail" aria-live="polite">
      {#if selected}
        <div class="label">Selected</div>
        <h3>{selected.name}{#if selected.ticker && selected.ticker !== selected.name}&nbsp;<span class="faint tk">{selected.ticker}</span>{/if}</h3>
        <div class="sub2"><i class="dot" style={dot(selected)}></i>{selected.sector}{#if selected.country}&nbsp;· {selected.country}{/if}{#if selected.currency}&nbsp;· {selected.currency}{/if}</div>
        <div class="big"><Money value={selected.value} currency={cur} size="l" /></div>
        <div class="sub2">{Number(selected.pct).toFixed(2)}% of your investments</div>
        {#if selected.via.length}
          <ul class="via">
            {#each selected.via as v}
              <li><span>{viaLabel(v)} <small>{Number(v.weight_pct).toFixed(2)}% of {v.path[0]}</small></span><Money value={v.value} currency={cur} /></li>
            {/each}
          </ul>
        {/if}
        <p class="note faint">Your value in each fund × the fund's disclosed weight, through every level.</p>
      {/if}
    </section>
    <section class="card funds">
      <h2 class="label">Holdings</h2>
      <table>
        <thead><tr><th>Holding</th><th class="r">Value</th><th class="r">File</th><th class="r">Names</th></tr></thead>
        <tbody>
          {#each x.funds as f (f.symbol)}
            <tr>
              <td>
                <span class="sym">{f.symbol}</span>
                <span class="sub2">{#if f.name}{f.name} · {/if}{#if f.children.length}holds {f.children.join(", ")}{:else if f.resolved}stocks directly{:else}no file, counted as one name{/if}{#if f.unresolved_children.length}&nbsp;<span class="warn">· no file for {f.unresolved_children.join(", ")}</span>{/if}</span>
              </td>
              <td class="r"><Money value={f.value} currency={cur} /></td>
              <td class="r">
                {#if f.resolved}
                  {f.as_of ? formatDate(f.as_of) : ""}{#if f.source && f.source !== "seed"}&nbsp;<span class="faint">({f.source})</span>{/if}
                  {#if f.coverage_pct != null && Number(f.coverage_pct) < 99.5}<span class="sub2 warn">{f.coverage_pct}% disclosed</span>{/if}
                {:else}
                  <span class="warn">none</span>
                {/if}
              </td>
              <td class="r num">{f.resolved ? f.constituents : "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if broken.length}
        <ul class="errs">{#each broken as [k, v]}<li><b>{k}</b> <span class="warn">{v}</span></li>{/each}</ul>
      {/if}
      <p class="faint note">
        {#if unresolvedFunds.length || missingChildren.length}
          A fund with no file counts as one line. Save its holdings CSV as <code>&lt;SYMBOL&gt;.csv</code>{#if missingChildren.length}&nbsp;(also {missingChildren.map((c) => `${c}.csv`).join(", ")}){/if} in the folder below{#if !lt.offline}, or set <code>settings.holdings_urls</code>{/if}.
        {/if}
        Folder: <code title={lt.holdings_dir}>{shortPath(lt.holdings_dir)}</code>
      </p>
    </section>
    </div>
  </div>

{/if}

<style>
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; color: var(--text-2); }
  .warn { color: var(--warn); }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; max-width: 60ch; }
  .chart { padding: 10px; }
  .legend { font-size: 11px; color: var(--text-3); margin: 0; padding: 10px 6px 2px; }
  .two { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(280px, 1fr); gap: 20px; align-items: start; }
  .side { display: flex; flex-direction: column; gap: 20px; position: sticky; top: 16px; }
  h2.label { margin: 0 0 8px; font-weight: 500; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; padding: 4px 0; }
  td { padding: 7px 0; border-top: 1px solid var(--border); vertical-align: top; }
  th.r, td.r { white-space: nowrap; padding-left: 14px; }
  th.bar, td.bar { width: 64px; padding-left: 14px; }
  td.bar { vertical-align: middle; }
  .top td:first-child { min-width: 0; max-width: 0; width: 100%; }
  tbody tr { cursor: pointer; }
  tbody tr:hover td, tbody tr.on td { color: var(--text); }
  .top tbody tr.on td:first-child { box-shadow: inset 2px 0 0 var(--accent); padding-left: 8px; }
  .funds tbody tr { cursor: default; }
  .funds td:first-child { min-width: 0; max-width: 0; width: 100%; }
  .funds td.r { white-space: nowrap; vertical-align: top; }
  .r { text-align: right; }
  .sub2 { display: block; font-size: 11px; color: var(--text-3); padding-left: 14px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .detail .sub2, .funds .sub2 { padding-left: 0; }
  .detail h3 + .sub2 { margin-top: 2px; }
  .sym { font-weight: 500; }
  .dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; margin-right: 7px; vertical-align: 1px; }
  .top td:first-child { padding-left: 0; }
  .wt { display: block; width: 50px; height: 6px; background: var(--surface-2); border-radius: 3px; overflow: hidden; }
  .wt i { display: block; height: 100%; background: var(--fill); }
  .detail h3 { margin: 2px 0 0; font-size: 17px; font-weight: 600; }
  .detail .tk { font-weight: 400; font-size: 13px; }
  .detail .big { margin-top: 12px; }
  .via { margin: 12px 0 0; padding: 0; list-style: none; font-size: 13px; }
  .via li { display: flex; justify-content: space-between; gap: 10px; padding: 6px 0; border-top: 1px solid var(--border); }
  .via small { color: var(--text-3); margin-left: 6px; }
  .note { font-size: 12px; margin: 12px 0 0; }
  .foot { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-top: 10px; font-size: 12px; min-height: 20px; }
  .more { font: inherit; font-size: 12px; color: var(--text-2); background: none; border: 1px solid var(--border); border-radius: 6px; padding: 3px 10px; cursor: pointer; }
  .more:hover { color: var(--text); }
  .more:focus-visible { outline: 2px solid var(--accent); }
  .errs { margin: 10px 0 0; padding: 0; list-style: none; font-size: 12px; }
  .errs li { padding: 4px 0; }
  code { font-family: var(--mono); font-size: 11px; }
  @media (max-width: 760px) {
    .two { grid-template-columns: 1fr; }
    th.bar, td.bar { display: none; }
    .side { position: static; }
  }
</style>
