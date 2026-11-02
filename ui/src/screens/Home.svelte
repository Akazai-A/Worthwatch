<script lang="ts">
  import { GROUPS, GROUP_LABEL, type AccountView, type LedgerView } from "../lib/api";
  import { formatDate, formatMoney } from "../lib/money";
  import { prefs } from "../lib/stores/prefs.svelte";
  import Money from "../lib/components/Money.svelte";
  import NetWorthChart from "../lib/charts/NetWorthChart.svelte";
  import Sparkline from "../lib/charts/Sparkline.svelte";
  import MarketLine from "../lib/components/MarketLine.svelte";

  let { view }: { view: LedgerView } = $props();
  const STALE_DAYS = 45;

  const cur = $derived(view.base_currency);
  const stale = $derived(view.accounts.filter((a) => a.days_stale == null || a.days_stale > STALE_DAYS));
  const byGroup = $derived(GROUPS.map((g) => ({ g, rows: view.accounts.filter((a) => a.group === g) })).filter((x) => x.rows.length));
  const assets = $derived(Number(view.totals.assets));
  const debt = $derived(Number(view.totals.liabilities));
  const liquid = $derived(assets - Number(view.totals.by_group.property ?? 0) - debt);
  const groupVal = (g: string) => Number(view.totals.by_group[g as keyof typeof view.totals.by_group] ?? 0);
  const share = (g: string) => (assets > 0 ? (groupVal(g) / assets) * 100 : 0);

  function spark(a: AccountView): number[] {
    const last = a.history[a.history.length - 1];
    if (!last) return [];
    const cut = new Date(last.date).getTime() - 90 * 86400000;
    const recent = a.history.filter((h) => new Date(h.date).getTime() >= cut);
    const sign = a.liability ? -1 : 1;
    return (recent.length >= 2 ? recent : a.history).map((h) => sign * Number(h.value));
  }
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
    {#if stale.length}
      <div class="banner">
        <span class="pill">{stale.length}</span>
        <span>{stale.length === 1 ? "account has" : "accounts have"} not been checked in for {STALE_DAYS} days.</span>
        <a class="act" href="#/check-in">Check in now →</a>
      </div>
    {/if}

    <section class="hero">
      <div class="label">Net worth</div>
      <Money value={view.totals.net_worth} currency={cur} size="xl" />
      <div class="sub">
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
    </section>

    <section class="card">
      <NetWorthChart series={view.series} events={view.events} currency={cur} blur={prefs.blur} />
    </section>

    <section class="card bars">
      <div class="bar">
        <div class="lab"><span>Assets</span><Money value={view.totals.assets} currency={cur} /></div>
        <div class="track">
          {#each GROUPS.filter((g) => g !== "debt") as g}
            {#if share(g) > 0}<i style="width:{share(g)}%;background:var(--g-{g})" title="{GROUP_LABEL[g]} {share(g).toFixed(0)}%"></i>{/if}
          {/each}
        </div>
      </div>
      <div class="bar">
        <div class="lab"><span>Debt</span><Money value={view.totals.liabilities} currency={cur} /></div>
        <div class="track"><i style="width:{assets > 0 ? Math.min(100, (debt / assets) * 100) : debt > 0 ? 100 : 0}%;background:var(--g-debt)"></i></div>
      </div>
      <div class="legend">
        {#each GROUPS.filter((g) => g !== "debt") as g}<span><i style="background:var(--g-{g})"></i>{GROUP_LABEL[g]} {share(g).toFixed(0)}%</span>{/each}
        <span><i style="background:var(--g-debt)"></i>Debt is {assets > 0 ? ((debt / assets) * 100).toFixed(0) : "–"}% of assets</span>
      </div>
    </section>

    <section class="tiles">
      <a class="card tile" href="#/portfolio"><div class="label">Investments</div><div class="v"><Money value={groupVal("investments")} currency={cur} size="l" /></div>{#if view.portfolio.day_change != null}<div class="d"><Money value={view.portfolio.day_change} currency={cur} signed tone="delta" /> <span class="faint">today</span></div>{/if}</a>
      <div class="card tile"><div class="label">Cash</div><div class="v"><Money value={groupVal("cash")} currency={cur} size="l" /></div></div>
      <div class="card tile"><div class="label">Debt</div><div class="v"><Money value={debt} currency={cur} size="l" /></div></div>
      <div class="card tile"><div class="label">Liquid net worth</div><div class="v"><Money value={liquid} currency={cur} size="l" /></div><div class="d faint">excludes property</div></div>
    </section>

    {#each byGroup as { g, rows }}
      <section class="card group">
        <h2 class="label">{GROUP_LABEL[g]}</h2>
        {#each rows as a}
          <a class="row" href="#/accounts/{encodeURIComponent(a.id)}">
            <span class="sd" class:old={a.days_stale != null && a.days_stale > STALE_DAYS} class:none={a.days_stale == null} title={a.as_of ? `updated ${formatDate(a.as_of, "long")}` : "never updated"}></span>
            <span class="name">{a.name}<span class="inst faint">{a.institution ?? ""}{a.currency !== cur ? ` · ${a.currency}` : ""}</span></span>
            <span class="spark"><Sparkline values={spark(a)} /></span>
            <span class="val">
              {#if a.missing_fx}<span class="warn" title="no FX rate for {a.currency}; add settings.fx in the ledger">no FX</span>
              {:else if a.missing_price}<span class="warn" title="a holding has no price yet">no price</span>
              {:else}<Money value={a.value} currency={cur} />{/if}
            </span>
            <span class="age" class:old={a.days_stale != null && a.days_stale > STALE_DAYS}>{a.days_stale == null ? "—" : `${a.days_stale}d`}</span>
          </a>
        {/each}
      </section>
    {/each}
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
  .banner { display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-radius: 8px; background: var(--surface); border: 1px solid var(--border); color: var(--text-2); font-size: 13px; }
  .pill { font-size: 11px; padding: 2px 7px; border-radius: 10px; background: var(--surface-2); color: var(--text-2); }
  .act { margin-left: auto; text-decoration: none; white-space: nowrap; }
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; }
  .chip { display: inline-flex; gap: 5px; align-items: baseline; }
  a.chip { text-decoration: none; color: inherit; }
  a.chip:hover .muted { color: var(--accent); }
  .bars { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .bar .lab { display: flex; justify-content: space-between; font-size: 12px; color: var(--text-2); margin-bottom: 6px; }
  .track { height: 8px; border-radius: 4px; background: var(--surface-2); overflow: hidden; display: flex; gap: 2px; }
  .track i { display: block; height: 100%; }
  .legend { grid-column: 1 / -1; display: flex; gap: 14px; font-size: 11px; color: var(--text-2); flex-wrap: wrap; }
  .legend i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; }
  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
  .tile { padding: 12px 14px; }
  .tile .v { margin-top: 2px; }
  .tile .d { font-size: 11px; }
  a.tile { color: inherit; text-decoration: none; }
  a.tile:hover .label { color: var(--accent); }
  .group h2 { margin: 2px 0 6px; font-weight: 500; }
  .row { display: grid; grid-template-columns: 10px 1fr 90px 120px 40px; align-items: center; gap: 12px; padding: 9px 0; border-top: 1px solid var(--border); color: inherit; text-decoration: none; }
  .row:first-of-type { border-top: 0; }
  .row:hover .name { color: var(--accent); }
  .sd { width: 7px; height: 7px; border-radius: 50%; background: var(--positive); }
  .sd.old { background: var(--warn); }
  .sd.none { background: var(--text-3); }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .inst { margin-left: 8px; font-size: 12px; }
  .val { text-align: right; }
  .warn { color: var(--warn); font-size: 12px; }
  .age { text-align: right; font-size: 11px; color: var(--text-3); }
  .age.old { color: var(--warn); }
  @media (max-width: 640px) {
    .bars, .tiles { grid-template-columns: 1fr 1fr; }
    .row { grid-template-columns: 10px 1fr 110px; }
    .row .spark, .row .age { display: none; }
  }
</style>
