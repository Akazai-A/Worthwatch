<script lang="ts">
  // Portfolio › Look-through: where the money actually is, through every fund.
  import type { Company, LookThroughView } from "../lib/api";
  import { formatDate, formatMoney } from "../lib/money";
  import { caveats, headlineParts, viaLabel, viaSummary } from "../lib/lookthrough";
  import Money from "../lib/components/Money.svelte";
  import Treemap from "../lib/charts/Treemap.svelte";

  let { lt }: { lt: LookThroughView } = $props();
  const cur = $derived(lt.base_currency);
  const x = $derived(lt.lookthrough);
  const top = $derived(x.companies.filter((c) => !c.undisclosed && c.key !== "__other__").slice(0, 25));
  let selectedKey = $state<string | null>(null);
  const selected = $derived<Company | null>(x.companies.find((c) => c.key === selectedKey) ?? top[0] ?? null);
  const parts = $derived(headlineParts(x.company_count, x.funds, x.top10_pct, x.effective_holdings));
  const warns = $derived(caveats(x.undisclosed, x.total, x.funds));
  const maxTop = $derived(top.length ? Number(top[0].value) : 1);
  const unresolvedFunds = $derived(x.funds.filter((f) => !f.resolved));
  const broken = $derived(Object.entries(lt.errors));
  const missingChildren = $derived([...new Set(x.funds.flatMap((f) => f.unresolved_children))]);
</script>

<section class="hero">
  <div class="label">Where the money actually is</div>
  <Money value={x.total} currency={cur} size="xl" />
  <div class="sub">
    {#each parts as p}<span class="chip">{p}</span>{/each}
    {#each warns as w}<span class="chip warn">{w}</span>{/each}
  </div>
</section>

{#if lt.snapshots.length === 0}
  <section class="card empty">
    <h2>No holdings files yet</h2>
    <p class="muted">
      Look-through needs each fund's holdings file. {#if lt.offline}Offline, so nothing is fetched:{:else}Press <em>Update holdings</em> above to fetch them from the issuer, or{/if}
      download the holdings CSV from the fund's page and save it as <code>{lt.holdings_dir}/&lt;SYMBOL&gt;.csv</code> (for example <code>XEQT.TO.csv</code>), then press <em>{lt.offline ? "Re-read folder" : "Update holdings"}</em>.
    </p>
    {#if broken.length}
      <ul class="errs">{#each broken as [k, v]}<li><b>{k}</b> <span class="warn">{v}</span></li>{/each}</ul>
    {/if}
  </section>
{:else}
  <section class="card chart">
    <Treemap companies={x.companies} currency={cur} selected={selected?.key ?? null} onselect={(k) => (selectedKey = k)} />
    <p class="legend">Grouped by sector, sized by your dollars; the stronger the tint, the larger the name within its block. Bonds and cash stay grey. Click a cell for the funds behind it.</p>
  </section>

  <div class="two">
    <section class="card">
      <h2 class="label">Top {top.length} companies</h2>
      <table>
        <thead><tr><th>Company</th><th class="r">Value</th><th class="r">Of total</th></tr></thead>
        <tbody>
          {#each top as c (c.key)}
            <tr class:on={selected?.key === c.key} onclick={() => (selectedKey = c.key)}>
              <td>
                {c.name}
                <span class="sub2">{c.sector} · {viaSummary(c)}</span>
              </td>
              <td class="r"><Money value={c.value} currency={cur} /></td>
              <td class="r"><span class="wt"><i style="width:{(Number(c.value) / maxTop) * 100}%"></i></span><span class="num">{Number(c.pct).toFixed(1)}%</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if x.company_count > top.length}
        <p class="faint note">… {(x.company_count - top.length).toLocaleString("en-CA")} more names, each under {Number(top[top.length - 1]?.pct ?? 0).toFixed(1)}%.</p>
      {/if}
    </section>

    <section class="card detail" aria-live="polite">
      {#if selected}
        <div class="label">Selected</div>
        <h3>{selected.name}{#if selected.ticker && selected.ticker !== selected.name}&nbsp;<span class="faint tk">{selected.ticker}</span>{/if}</h3>
        <div class="sub2">{selected.sector}{#if selected.country}&nbsp;· {selected.country}{/if}{#if selected.currency}&nbsp;· {selected.currency}{/if}</div>
        <div class="big"><Money value={selected.value} currency={cur} size="l" /></div>
        <div class="sub2">{Number(selected.pct).toFixed(2)}% of your investments</div>
        {#if selected.via.length}
          <ul class="via">
            {#each selected.via as v}
              <li><span>{viaLabel(v)} <small>{Number(v.weight_pct).toFixed(2)}% of {v.path[0]}</small></span><Money value={v.value} currency={cur} /></li>
            {/each}
          </ul>
        {/if}
        <p class="note faint">Click any cell or row. Weights come from each issuer's holdings file; dollars are your value in the fund × that weight, through every level.</p>
      {/if}
    </section>
  </div>

  <section class="card funds">
    <h2 class="label">Funds</h2>
    <table>
      <thead><tr><th>Holding</th><th class="r">Value</th><th>Holdings file</th><th class="r">Names</th><th>Expands into</th></tr></thead>
      <tbody>
        {#each x.funds as f (f.symbol)}
          <tr>
            <td><span class="sym">{f.symbol}</span>{#if f.name}<span class="sub2">{f.name}</span>{/if}</td>
            <td class="r"><Money value={f.value} currency={cur} /></td>
            <td>
              {#if f.resolved}
                {f.as_of ? formatDate(f.as_of) : ""}{#if f.source && f.source !== "seed"}&nbsp;<span class="faint">({f.source})</span>{/if}
                {#if f.coverage_pct != null && Number(f.coverage_pct) < 99.5}<span class="sub2 warn">{f.coverage_pct}% disclosed</span>{/if}
              {:else}
                <span class="warn">none</span><span class="sub2">treated as one security</span>
              {/if}
            </td>
            <td class="r num">{f.resolved ? f.constituents : "—"}</td>
            <td class="faint">{f.children.length ? f.children.join(", ") : f.resolved ? "stocks directly" : ""}{#if f.unresolved_children.length}<span class="sub2 warn">no file for {f.unresolved_children.join(", ")}</span>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if broken.length}
      <ul class="errs">{#each broken as [k, v]}<li><b>{k}</b> <span class="warn">{v}</span></li>{/each}</ul>
    {/if}
    <p class="faint note">
      {#if unresolvedFunds.length || missingChildren.length}
        A fund without a file counts as one security. {#if !lt.offline}iShares funds are fetched by ticker; for others{:else}Offline, so{/if} download the holdings file from the issuer and save it as
        <code>{lt.holdings_dir}/&lt;SYMBOL&gt;.csv</code>{#if missingChildren.length}&nbsp;(for the funds inside a fund too: {missingChildren.map((c) => `${c}.csv`).join(", ")}){/if}, or put its address under <code>settings.holdings_urls</code> in the ledger.
      {/if}
      Snapshots are kept per fund per date in <code>{lt.holdings_dir}</code> and never rewritten. Stocks you hold directly can be given a sector and country under <code>securities</code> in the ledger.
    </p>
  </section>
{/if}

<style>
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; color: var(--text-2); }
  .warn { color: var(--warn); }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; max-width: 60ch; }
  .chart { padding: 10px; }
  .legend { font-size: 11px; color: var(--text-3); margin: 0; padding: 10px 6px 2px; }
  .two { display: grid; grid-template-columns: 1.1fr 1fr; gap: 20px; }
  h2.label { margin: 0 0 8px; font-weight: 500; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; padding: 4px 0; }
  td { padding: 7px 0; border-top: 1px solid var(--border); vertical-align: top; }
  tbody tr { cursor: pointer; }
  tbody tr:hover td, tbody tr.on td { color: var(--text); }
  tbody tr.on td:first-child { box-shadow: inset 2px 0 0 var(--accent); padding-left: 8px; }
  .funds tbody tr { cursor: default; }
  .funds th, .funds td { padding-right: 14px; }
  .funds th:last-child, .funds td:last-child { padding-right: 0; }
  .funds td.r { white-space: nowrap; }
  .r { text-align: right; }
  .sub2 { display: block; font-size: 11px; color: var(--text-3); padding-left: 0; }
  .funds td .sub2 { padding-left: 0; }
  .sym { font-weight: 500; }
  .wt { display: inline-block; width: 56px; height: 6px; background: var(--surface-2); border-radius: 3px; vertical-align: middle; margin-right: 8px; overflow: hidden; }
  .wt i { display: block; height: 100%; background: var(--fill); }
  .detail h3 { margin: 2px 0 0; font-size: 17px; font-weight: 600; }
  .detail .tk { font-weight: 400; font-size: 13px; }
  .detail .big { margin-top: 12px; }
  .via { margin: 12px 0 0; padding: 0; list-style: none; font-size: 13px; }
  .via li { display: flex; justify-content: space-between; gap: 10px; padding: 6px 0; border-top: 1px solid var(--border); }
  .via small { color: var(--text-3); margin-left: 6px; }
  .note { font-size: 12px; margin: 12px 0 0; }
  .errs { margin: 10px 0 0; padding: 0; list-style: none; font-size: 12px; }
  .errs li { padding: 4px 0; }
  code { font-family: var(--mono); font-size: 11px; }
  @media (max-width: 760px) {
    .two { grid-template-columns: 1fr; }
    th:nth-child(3), td:nth-child(3) { display: none; }
    .funds th:nth-child(3), .funds td:nth-child(3) { display: table-cell; }
    .funds th:nth-child(5), .funds td:nth-child(5) { display: none; }
  }
</style>
