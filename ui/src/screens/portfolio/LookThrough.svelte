<script lang="ts">
  import PrivacyToggle from "../../lib/components/shell/PrivacyToggle.svelte";
  // Portfolio › Look-through: where the money actually is, through every fund.
  import type { Company, LookThroughView } from "../../lib/api";
  import { formatPct } from "../../lib/format/money";
  import { caveats, familyHue, familyOf, FLAG, headlineParts, overlapPairs, shortPath, viaLabel } from "../../lib/portfolio/lookthrough";
  import Lineage from "../../lib/components/portfolio/Lineage.svelte";
  import FundsCard from "../../lib/components/portfolio/FundsCard.svelte";
  import ExposureBars from "../../lib/components/portfolio/ExposureBars.svelte";
  import Money from "../../lib/components/amounts/Money.svelte";
  import Treemap from "../../lib/charts/Treemap.svelte";
  import { hashFor } from "../../lib/stores/route.svelte";

  // cash: investment accounts that hold cash rather than funds (an FHSA not invested yet).
  let { lt, cash = [] }: { lt: LookThroughView; cash?: { name: string; value: number }[] } = $props();
  const cashTotal = $derived(cash.reduce((s, c) => s + c.value, 0));
  const cur = $derived(lt.base_currency);
  const x = $derived(lt.lookthrough);
  // The top 50, in a list that scrolls inside its card so the page never grows with it.
  const MAX = 50;
  const top = $derived(x.companies.filter((c) => !c.undisclosed && c.key !== "__other__").slice(0, MAX));
  let selectedKey = $state<string | null>(null);
  const selected = $derived<Company | null>(x.companies.find((c) => c.key === selectedKey) ?? top[0] ?? null);
  const parts = $derived(headlineParts(x.company_count, x.funds, x.top10_pct, x.effective_holdings));
  // Overlap shows up only as a flag: two funds this much alike are worth knowing about.
  const pairs = $derived(overlapPairs(lt.overlap));
  const twins = $derived(pairs.filter((p) => p.pct >= FLAG.pair));
  const warns = $derived(caveats(x.undisclosed, x.total, x.funds));
  const maxTop = $derived(top.length ? Number(top[0].value) : 1);
  const unresolvedFunds = $derived(x.funds.filter((f) => !f.resolved));
  const broken = $derived(Object.entries(lt.errors));
  const missingChildren = $derived([...new Set(x.funds.flatMap((f) => f.unresolved_children))]);
  const dot = (c: Company) => `background:var(${familyHue(familyOf(c.slot))})`;
</script>

<section class="hero">
  <h1 class="label">Where the money actually is</h1>
  <Money value={Number(x.total) + cashTotal} currency={cur} size="xl" />
  <div class="sub">
    <PrivacyToggle />
    {#each cash as c (c.name)}<span class="chip"><Money value={c.value} currency={cur} />&nbsp;<span class="muted">cash in {c.name}, not invested yet</span></span>{/each}
    {#each parts as p}<span class="chip">{p}</span>{/each}
    {#each twins as t (`${t.a}|${t.b}`)}<span class="chip warn" title="{t.shared.slice(0, 5).map((n) => n.name).join(', ')}…">{t.a} and {t.b} are {t.pct.toFixed(0)}% the same</span>{/each}
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
      <div class="scroll">
        <table class="top">
          <thead><tr><th>Company</th><th class="r">Value</th><th class="bar"></th><th class="r">Of total</th><th class="go"></th></tr></thead>
          <tbody>
            {#each top as c, i (c.key)}
              <tr class:on={selected?.key === c.key} onclick={() => (selectedKey = c.key)}>
                <td><span class="rank num">{i + 1}</span><i class="dot" style={dot(c)}></i>{c.name}<span class="sub2">{c.sector}</span></td>
                <td class="r"><Money value={c.value} currency={cur} /></td>
                <td class="bar"><span class="wt"><i style="width:{(Number(c.value) / maxTop) * 100}%"></i></span></td>
                <td class="r num">{formatPct(c.pct)}</td>
                <td class="go"><a href={hashFor({ name: "company", key: c.key })} aria-label="Open {c.name}" onclick={(e) => e.stopPropagation()}>›</a></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      {#if x.company_count > top.length}
        <p class="foot faint">{(x.company_count - top.length).toLocaleString("en-CA")} more names, each under {formatPct(top[top.length - 1]?.pct ?? 0)}</p>
      {/if}
    </section>

    <section class="card detail" aria-live="polite">
      {#if selected}
        <div class="dh"><span class="label">Selected</span><a class="open" href={hashFor({ name: "company", key: selected.key })}>Open page ›</a></div>
        <h3>{selected.name}{#if selected.ticker && selected.ticker !== selected.name}&nbsp;<span class="faint tk">{selected.ticker}</span>{/if}</h3>
        <div class="sub2"><i class="dot" style={dot(selected)}></i>{selected.sector}{#if selected.country}&nbsp;· {selected.country}{/if}{#if selected.currency}&nbsp;· {selected.currency}{/if}</div>
        <div class="big"><Money value={selected.value} currency={cur} size="l" /></div>
        <div class="sub2">{formatPct(selected.pct, 2)} of your investments</div>
        {#if selected.via.length}
          <Lineage via={selected.via} name={selected.name} color="var({familyHue(familyOf(selected.slot))})" currency={cur} label={viaLabel} />
        {/if}
        <p class="note faint">Your value in each fund × the fund's disclosed weight, through every level.</p>
      {/if}
    </section>
  </div>

  <FundsCard funds={x.funds} companies={x.companies} {pairs} total={Number(x.total)} currency={cur} asOf={lt.as_of} />
  {#if broken.length || unresolvedFunds.length || missingChildren.length}
    <section class="card fix">
      {#if broken.length}<ul class="errs">{#each broken as [k, v]}<li><b>{k}</b> <span class="warn">{v}</span></li>{/each}</ul>{/if}
      <p class="faint note">
        A fund with no file counts as one line. Save its holdings CSV as <code>&lt;SYMBOL&gt;.csv</code>{#if missingChildren.length}&nbsp;(also {missingChildren.map((c) => `${c}.csv`).join(", ")}){/if} in <code title={lt.holdings_dir}>{shortPath(lt.holdings_dir)}</code>{#if !lt.offline}, or set <code>settings.holdings_urls</code>{/if}.
      </p>
    </section>
  {/if}
{/if}

<ExposureBars {lt} />

<style>
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; color: var(--text-2); }
  .warn { color: var(--warn); }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; max-width: 60ch; }
  .chart { padding: 10px; }
  .legend { font-size: 11px; color: var(--text-3); margin: 0; padding: 10px 6px 2px; }
  .two { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(280px, 1fr); gap: 20px; align-items: start; }
  .detail { position: sticky; top: 16px; }
  /* About ten rows tall; the rest scroll inside the card. */
  .scroll { max-height: 520px; overflow-y: auto; overscroll-behavior: contain; box-shadow: inset 0 -12px 10px -10px rgba(0, 0, 0, 0.35); }
  .scroll thead th { position: sticky; top: 0; z-index: 1; background: var(--bg); }
  .rank { display: inline-block; width: 22px; font-size: 11px; color: var(--text-3); }
  h2.label { margin: 0 0 8px; font-weight: 500; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 12px; padding: 4px 0; }
  td { padding: 7px 0; border-top: 1px solid var(--border); vertical-align: top; }
  th.r, td.r { white-space: nowrap; padding-left: 14px; }
  th.bar, td.bar { width: 64px; padding-left: 14px; }
  td.bar { vertical-align: middle; }
  .top td:first-child { min-width: 0; max-width: 0; width: 100%; }
  tbody tr { cursor: pointer; }
  tbody tr:hover td, tbody tr.on td { color: var(--text); }
  .top tbody tr.on td:first-child { box-shadow: inset 2px 0 0 var(--accent); }
  .r { text-align: right; }
  .sub2 { display: block; font-size: 11px; color: var(--text-3); padding-left: 36px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .detail .sub2 { padding-left: 0; }
  .detail h3 + .sub2 { margin-top: 2px; }
  .dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; margin-right: 7px; vertical-align: 1px; }
  .top td:first-child, .top th:first-child { padding-left: 8px; }
  .wt { display: block; width: 50px; height: 6px; background: var(--surface-2); border-radius: 3px; overflow: hidden; }
  .wt i { display: block; height: 100%; background: var(--fill); }
  th.go, td.go { width: 22px; padding-left: 8px; text-align: right; }
  td.go a { color: var(--text-3); text-decoration: none; padding: 2px 4px; }
  td.go a:hover, td.go a:focus-visible { color: var(--accent); }
  .dh { display: flex; justify-content: space-between; align-items: baseline; }
  .open { font-size: 12px; color: var(--text-2); text-decoration: none; }
  .open:hover { color: var(--accent); }
  .detail h3 { margin: 2px 0 0; font-size: 17px; font-weight: 600; }
  .detail .tk { font-weight: 400; font-size: 13px; }
  .detail .big { margin-top: 12px; }
  .note { font-size: 12px; margin: 12px 0 0; }
  .foot { margin: 10px 0 0; font-size: 12px; }
  .fix .note { margin: 0; }
  .errs { margin: 0 0 10px; padding: 0; list-style: none; font-size: 12px; }
  .errs li { padding: 4px 0; }
  code { font-family: var(--mono); font-size: 11px; }
  @media (max-width: 760px) {
    .two { grid-template-columns: 1fr; }
    th.bar, td.bar { display: none; }
    .detail { position: static; }
    .scroll { max-height: 440px; }
  }
</style>
