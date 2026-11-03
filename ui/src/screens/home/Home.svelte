<script lang="ts">
  import PrivacyToggle from "../../lib/components/shell/PrivacyToggle.svelte";
  import { GROUPS, GROUP_LABEL, getChanges, getGrowth, type AccountView, type Changes, type GrowthPoint, type LedgerView } from "../../lib/api";
  import { formatDate, formatMoney, formatSigned } from "../../lib/format/money";
  import { parts } from "../../lib/portfolio/changes";
  import SplitStrip from "../../lib/components/amounts/SplitStrip.svelte";
  import { prefs } from "../../lib/stores/prefs.svelte";
  import Money from "../../lib/components/amounts/Money.svelte";
  import NetWorthChart from "../../lib/charts/NetWorthChart.svelte";
  import MarketLine from "../../lib/components/portfolio/MarketLine.svelte";

  let { view }: { view: LedgerView } = $props();
  const STALE_DAYS = 45;

  const cur = $derived(view.base_currency);
  const stale = $derived(view.accounts.filter((a) => a.days_stale == null || a.days_stale > STALE_DAYS));
  const byGroup = $derived(GROUPS.map((g) => ({ g, rows: view.accounts.filter((a) => a.group === g) })).filter((x) => x.rows.length));
  const assets = $derived(Number(view.totals.assets));
  const debt = $derived(Number(view.totals.liabilities));
  const groupVal = (g: string) => Number(view.totals.by_group[g as keyof typeof view.totals.by_group] ?? 0);
  // Liquid: what you could reach without selling the home. The mortgage goes with the property.
  const otherDebt = $derived(view.accounts.filter((a) => a.liability && a.kind !== "mortgage").reduce((s, a) => s + Number(a.value ?? 0), 0));
  const liquid = $derived(groupVal("cash") + groupVal("investments") - otherDebt);

  // Row visual: how big the account is (share of assets) and how it moved in the last 30 days.
  // Debt is flipped so paying it down reads as a gain, like on Changes.
  function month(a: AccountView): number | null {
    const h = a.history;
    const last = h[h.length - 1];
    if (!last || h.length < 2) return null;
    const cut = new Date(last.date).getTime() - 30 * 86400000;
    let base = h[0];
    for (const p of h) if (new Date(p.date).getTime() <= cut) base = p;
    if (base === last) return null;
    const d = Number(last.value) - Number(base.value);
    return a.liability ? -d : d;
  }
  const moves = $derived(new Map(view.accounts.map((a) => [a.id, month(a)])));
  const maxMove = $derived(Math.max(1, ...[...moves.values()].map((m) => Math.abs(m ?? 0))));
  const sizeOf = (a: AccountView) => (assets > 0 ? Math.min(100, (Math.abs(Number(a.value ?? 0)) / assets) * 100) : 0);
  const subtotal = (g: string, rows: AccountView[]) => (g === "debt" ? debt : rows.reduce((s, a) => s + Number(a.value ?? 0), 0));
  const isOld = (a: AccountView) => a.days_stale == null || a.days_stale > STALE_DAYS;

  // "Why it moved" under the hero: the last check-in period, split into its parts.
  let why = $state<Changes | null>(null);
  let whyKey = -1;
  $effect(() => {
    const v = view.version;
    const from = view.deltas.previous?.from;
    if (v === whyKey || !from) return;
    whyKey = v;
    // Same period as the headline's "since …", so the parts add up to that figure.
    getChanges({ from }).then((r) => { why = r.changes ?? null; }, () => { why = null; });
  });
  const whyParts = $derived(why ? parts(why) : []);

  // Added vs grown: the chart's default view once this loads.
  let growth = $state<GrowthPoint[] | null>(null);
  let growthKey = -1;
  $effect(() => {
    const v = view.version;
    if (v === growthKey) return;
    growthKey = v;
    getGrowth().then((g) => { growth = g; }, () => { growth = null; });
  });
</script>

<div class="home">
  <div class="top faint"><span>as of {formatDate(view.as_of, "long")} · {cur}</span>{#if view.accounts.length}<MarketLine {view} />{/if}</div>

  {#if view.accounts.length === 0}
    <section class="card empty">
      <h2>Add your first account</h2>
      <p class="muted">Worthwatch tracks what you own and owe from numbers you type in once a month. Start by adding your accounts, then enter today's balances.</p>
      <a class="btn" href="#/check-in">Go to Check-in</a>
      <p class="faint small">Or edit <code>{view.ledger_path}</code> by hand; the app reloads when you save. <code>worthwatch seed</code> writes a sample ledger to look at.</p>
    </section>
  {:else}
    <section class="hero">
      <h1 class="label">Net worth</h1>
      <Money value={view.totals.net_worth} currency={cur} size="xl" />
      <div class="sub">
        <PrivacyToggle />
        {#if view.deltas.previous}
          <a class="chip link" href="#/changes" title="What moved it: Changes"><Money value={view.deltas.previous.amount} currency={cur} signed tone="delta" />
            <span class="muted">{#if view.deltas.previous.pct != null}({view.deltas.previous.pct}%)&nbsp;{/if}since {formatDate(view.deltas.previous.from)}</span></a>
        {/if}
        {#if view.deltas.ytd}
          <span class="chip"><Money value={view.deltas.ytd.amount} currency={cur} signed /><span class="muted">YTD</span></span>
        {/if}
        {#if view.deltas.one_year?.pct != null}
          <span class="chip"><span class="num">{Number(view.deltas.one_year.pct) > 0 ? "+" : ""}{view.deltas.one_year.pct}%</span><span class="muted">1Y</span></span>
        {/if}
        {#if !view.deltas.previous && !view.deltas.ytd}
          <span class="muted">One check-in so far. The change line appears after the next one.</span>
        {/if}
      </div>
      {#if whyParts.length}
        <a class="why" href="#/changes" title="What moved it: Changes">
          <SplitStrip parts={whyParts} currency={cur} height={6} legend blur={prefs.blur} />
          <span class="go">Why it moved →</span>
        </a>
      {/if}
    </section>

    <section class="card">
      <NetWorthChart series={view.series} events={view.events} currency={cur} blur={prefs.blur} {growth} />
    </section>

    <section class="tiles">
      <a class="card tile" style="--gc:var(--g-investments)" href="#/portfolio"><div class="label">Investments</div><div class="v"><Money value={groupVal("investments")} currency={cur} size="l" /></div>{@render shareOf(groupVal("investments"))}{#if view.portfolio.day_change != null}<div class="d"><Money value={view.portfolio.day_change} currency={cur} signed tone="delta" /> <span class="faint">today</span></div>{/if}</a>
      <div class="card tile" style="--gc:var(--g-cash)"><div class="label">Cash</div><div class="v"><Money value={groupVal("cash")} currency={cur} size="l" /></div>{@render shareOf(groupVal("cash"))}</div>
      <div class="card tile" style="--gc:var(--g-debt)"><div class="label">Debt</div><div class="v"><Money value={debt} currency={cur} size="l" /></div>{@render shareOf(debt)}</div>
      <div class="card tile" style="--gc:var(--text-2)" title="Cash and investments, less debt other than the mortgage. The condo and its mortgage sit apart."><div class="label">Liquid net worth</div><div class="v"><Money value={liquid} currency={cur} size="l" /></div>{@render shareOf(liquid, "without the condo and mortgage")}</div>
    </section>

{#snippet shareOf(v: number, note = "")}
  <div class="sh"><i style="width:{assets > 0 ? Math.max(0, Math.min(100, (v / assets) * 100)) : 0}%"></i></div>
  <div class="d faint">{assets > 0 ? Math.round((v / assets) * 100) : 0}% of assets{#if note}<br />{note}{/if}</div>
{/snippet}

    {#each byGroup as { g, rows }}
      <section class="group" style="--gc:var(--g-{g})">
        <div class="gh"><h2 class="label">{GROUP_LABEL[g]}</h2><Money value={subtotal(g, rows)} currency={cur} /></div>
        <div class="rows">
          {#each rows as a, ri}
            {@const m = moves.get(a.id) ?? null}
            <a class="row" style="--i:{Math.min(ri, 8)}" href="#/accounts/{encodeURIComponent(a.id)}">
              <span class="name">
                <span class="n">{a.name}<span class="inst faint">{a.institution ?? ""}{a.currency !== cur ? ` · ${a.currency}` : ""}</span></span>
                <span class="size" title="{sizeOf(a).toFixed(0)}% of assets"><i style="width:{sizeOf(a)}%"></i></span>
              </span>
              <span class="move" title={m == null ? "no value 30 days back" : `${formatSigned(m, cur)} in the last 30 days${a.liability ? " (paid down counts as a gain)" : ""}`}>
                <span class="dv" aria-hidden="true"><i class:down={(m ?? 0) < 0} style="width:{(Math.abs(m ?? 0) / maxMove) * 50}%"></i></span>
                <span class="mv" class:down={(m ?? 0) < 0}>{#if m != null && Math.abs(m) >= 0.5}<Money value={m} currency={cur} signed />{/if}</span>
              </span>
              <span class="val">
                {#if a.missing_fx}<span class="warn" title="no FX rate for {a.currency}; add settings.fx in the ledger">no FX</span>
                {:else if a.missing_price}<span class="warn" title="a holding has no price yet">no price</span>
                {:else}<Money value={a.value} currency={cur} />{/if}
              </span>
              <span class="age" title={a.as_of ? `updated ${formatDate(a.as_of, "long")}` : "never updated"}>
                {#if a.days_stale == null}<span class="old">never</span>
                {:else if isOld(a)}<i class="tick hatch"></i><span class="old">{a.days_stale}d</span>
                {:else if a.days_stale > 0}<i class="tick" style="width:{Math.max(3, (a.days_stale / STALE_DAYS) * 16)}px"></i>{/if}
              </span>
            </a>
          {/each}
        </div>
      </section>
    {/each}

    {#if stale.length}
      <p class="stale">
        {#if stale.length === 1}{stale[0].name} hasn't been checked in {stale[0].days_stale == null ? "yet" : `for ${stale[0].days_stale} days`}.
        {:else}{stale.length} accounts haven't been checked in for {STALE_DAYS}+ days: {stale.map((a) => a.name).join(", ")}.{/if}
        <a href="#/check-in">Check in now →</a>
      </p>
    {/if}
  {/if}
</div>

<style>
  .home { display: flex; flex-direction: column; gap: 20px; }
  .top { display: flex; justify-content: space-between; align-items: center; gap: 12px; font-size: 12px; flex-wrap: wrap; }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; max-width: 52ch; }
  .small { font-size: 12px; }
  .btn { background: var(--accent); color: var(--bg); border: 0; border-radius: 8px; padding: 9px 16px; font-weight: 600; text-decoration: none; }
  code { font-family: var(--mono); font-size: 12px; }
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; }
  .chip { display: inline-flex; gap: 5px; align-items: baseline; }
  a.chip { text-decoration: none; color: inherit; }
  a.chip:hover .muted { color: var(--accent); }
  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
  /* Ledger tiles: no box, a rule on top in the group's colour. */
  .tile { padding: 10px 0 0; border-top: 2px solid var(--gc, var(--text-3)); }
  .tile .v { margin-top: 2px; }
  .sh { height: 4px; margin: 10px 0 4px; background: var(--surface-2); border-radius: 2px; overflow: hidden; }
  .sh i { display: block; height: 100%; background: var(--gc, var(--text-2)); }
  .tile .d { font-size: 12px; }
  a.tile { color: inherit; text-decoration: none; }
  a.tile:hover .label { color: var(--accent); }
  /* Groups: a margin column with the name and subtotal, rows on the right. */
  .group { display: grid; grid-template-columns: 150px 1fr; gap: 20px; border-top: 1px solid var(--border); padding-top: 14px; }
  .gh { display: flex; flex-direction: column; gap: 2px; font-size: 13px; color: var(--text-2); }
  .gh h2 { margin: 0; }
  .row { display: grid; grid-template-columns: minmax(0, 1fr) 190px 110px 56px; align-items: center; gap: 14px; padding: 8px 0 8px 12px; border-top: 1px solid var(--border); color: inherit; text-decoration: none; box-shadow: inset 3px 0 0 var(--gc); }
  .row:first-child { border-top: 0; }
  .row:hover .n { color: var(--accent); }
  .name { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .n { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .inst { margin-left: 8px; font-size: 12px; }
  /* Share of assets: a hairline track, so a small account reads as a short bar, not a dot. */
  .size { display: block; height: 2px; background: var(--border); border-radius: 1px; max-width: 360px; }
  .size i { display: block; height: 100%; min-width: 3px; background: var(--gc); border-radius: 1px; }
  .move { display: grid; grid-template-columns: 1fr 72px; gap: 8px; align-items: center; }
  .dv { position: relative; height: 6px; background: var(--surface-2); border-radius: 3px; }
  .dv::before { content: ""; position: absolute; left: 50%; top: -2px; bottom: -2px; width: 1px; background: var(--text-3); }
  .dv i { position: absolute; left: 50%; top: 0; bottom: 0; background: var(--positive); border-radius: 0 3px 3px 0; }
  .dv i.down { left: auto; right: 50%; background: var(--negative); border-radius: 3px 0 0 3px; }
  .mv { font-size: 12px; text-align: right; color: var(--positive); }
  .mv.down { color: var(--negative); }
  .val { text-align: right; }
  .warn { color: var(--warn); font-size: 12px; }
  /* Age: nothing when fresh, a tick that grows with age, hatched with the day count once stale. */
  .age { display: flex; align-items: center; justify-content: flex-end; gap: 5px; font-size: 12px; }
  .tick { display: block; height: 4px; border-radius: 1px; background: var(--text-3); }
  .tick.hatch { width: 16px; background: repeating-linear-gradient(-45deg, var(--warn) 0 2px, transparent 2px 4px); }
  .old { color: var(--warn); }
  .why { display: flex; align-items: flex-end; gap: 16px; margin-top: 14px; color: inherit; text-decoration: none; font-size: 12px; }
  .why :global(.strip) { flex: 1; max-width: 560px; }
  .go { color: var(--text-2); white-space: nowrap; }
  .why:hover .go { color: var(--accent); }
  .stale { margin: 4px 0 0; font-size: 13px; color: var(--text-2); display: flex; gap: 12px; flex-wrap: wrap; }
  .stale a { margin-left: auto; }
  .row { transition: background-color 120ms ease-out; }
  .row:hover { background: color-mix(in oklab, var(--gc) 7%, transparent); }
  .sh i, .size i, .dv i { transition: width 360ms var(--ease-out); }
  /* Bars grow out of their baseline on arrival (delta bars out of the zero line), a row at a time. */
  @media (prefers-reduced-motion: no-preference) {
    .sh i, .size i, .dv i { transform-origin: left center; animation: grow 620ms var(--ease-out) both; animation-delay: calc(80ms + var(--i, 0) * 30ms); }
    .dv i.down { transform-origin: right center; }
  }
  @keyframes grow { from { transform: scaleX(0); } }
  /* Tablet: the group name moves above its rows, so the rows keep their columns without scrolling sideways. */
  @media (max-width: 960px) {
    .group { grid-template-columns: 1fr; gap: 6px; }
    .gh { flex-direction: row; justify-content: space-between; }
  }
  @media (max-width: 640px) {
    .tiles { grid-template-columns: 1fr 1fr; }
    .group { grid-template-columns: 1fr; gap: 6px; }
    .gh { flex-direction: row; justify-content: space-between; }
    .row { grid-template-columns: minmax(0, 1fr) 96px 40px; gap: 10px; }
    .move { grid-template-columns: 1fr; }
    .move .dv { display: none; }
    .val { grid-column: 2; grid-row: 1; }
    .move { grid-column: 2; grid-row: 2; }
    .age { grid-column: 3; grid-row: 1; }
  }
</style>
