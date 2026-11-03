<script lang="ts">
  import PrivacyToggle from "../../lib/components/shell/PrivacyToggle.svelte";
  import type { LedgerView, SecurityRow } from "../../lib/api";
  import { formatDate, formatPlain, formatUnits, formatPct } from "../../lib/format/money";
  import { providerLabel } from "../../lib/portfolio/market";
  import { costByAccount } from "../../lib/portfolio/costgain";
  import Money from "../../lib/components/amounts/Money.svelte";
  import MarketLine from "../../lib/components/portfolio/MarketLine.svelte";
  import HoldingsLine from "../../lib/components/portfolio/HoldingsLine.svelte";
  import PortfolioTabs from "../../lib/components/portfolio/PortfolioTabs.svelte";
  import LookThrough from "./LookThrough.svelte";
  import Exposure from "./Exposure.svelte";
  import Overlap from "./Overlap.svelte";
  import { lookthrough } from "../../lib/stores/portfolio/lookthrough.svelte";
  import type { PortfolioTab } from "../../lib/stores/route.svelte";

  let { view, tab = "positions" }: { view: LedgerView; tab?: PortfolioTab } = $props();
  // The look-through view is its own call; keep it in step with the ledger version while a tab needs it.
  $effect(() => {
    if (tab !== "positions") void lookthrough.ensure(view.version);
  });
  const lt = $derived(lookthrough.current);
  const cur = $derived(view.base_currency);
  const p = $derived(view.portfolio);
  const STALE_DAYS = 4;
  let open = $state<Record<string, boolean>>({});
  // Book cost vs gain per account; the gap to Home's investments total is accounts with no positions.
  const costs = $derived(costByAccount(p.securities));
  const maxCost = $derived(Math.max(1, ...costs.map((r) => Math.max(r.value, r.cost ?? 0))));
  const gap = $derived(Number(view.totals.by_group.investments ?? 0) - Number(p.total));
  const cashOnly = $derived(view.accounts.filter((a) => a.group === "investments" && a.positions == null).map((a) => a.name));

  function toggle(sym: string) {
    open[sym] = !open[sym];
  }
  function priceAge(r: SecurityRow): number | null {
    if (!r.price_date) return null;
    return Math.round((new Date(view.as_of).getTime() - new Date(r.price_date).getTime()) / 86400000);
  }
  function positionAccounts(): number {
    return view.accounts.filter((a) => a.positions != null).length;
  }
</script>

<div class="portfolio">
  <div class="top faint">
    <span>as of {formatDate(view.as_of, "long")}</span>
    {#if tab === "positions" || !lt}<MarketLine {view} />{:else}<HoldingsLine {lt} />{/if}
  </div>
  <PortfolioTabs {tab} />

  {#if tab !== "positions"}
    {#if lookthrough.error && !lt}
      <p class="error">Could not load look-through: {lookthrough.error}</p>
    {:else if !lt}
      <p class="faint">Loading…</p>
    {:else if tab === "lookthrough"}
      <LookThrough {lt} />
    {:else if tab === "exposure"}
      <Exposure {lt} />
    {:else}
      <Overlap {lt} />
    {/if}
  {:else if p.securities.length === 0}
    <section class="card empty">
      <h2>No positions yet</h2>
      {#if positionAccounts() === 0}
        <p class="muted">Add an investment account with holdings on <a href="#/check-in">Check-in</a> (list the symbols when you add it), or paste a holdings export from your broker there.</p>
      {:else}
        <p class="muted">Your investment accounts have no holdings. On <a href="#/check-in">Check-in</a>, use <em>Paste holdings</em> on the account, or type units next to each symbol.</p>
      {/if}
    </section>
  {:else}
    <section class="hero">
      <div class="label">Investments</div>
      <Money value={p.total} currency={cur} size="xl" />
      <div class="sub">
        <PrivacyToggle />
        {#if p.day_change != null}
          <span class="chip"><Money value={p.day_change} currency={cur} signed tone="delta" /><span class="muted">since previous close</span></span>
        {/if}
        {#if p.gain != null}
          <span class="chip"><Money value={p.gain} currency={cur} signed tone="delta" /><span class="muted">vs book cost{#if p.gain_pct != null}&nbsp;({Number(p.gain_pct) > 0 ? "+" : ""}{p.gain_pct}%){/if}</span></span>
        {/if}
        {#if p.unpriced.length}
          <span class="chip warn">{p.unpriced.length} {p.unpriced.length === 1 ? "holding has" : "holdings have"} no price: {p.unpriced.join(", ")}</span>
        {/if}
      </div>
    </section>

    {#if costs.length}
      <section class="card cost" aria-label="What you put in and what it grew">
        <div class="ch"><h2 class="label">What you put in, and what it grew</h2><span class="key"><span><i class="c"></i>book cost</span><span><i class="g"></i>gain</span>{#if costs.some((r) => (r.gain ?? 0) < 0)}<span><i class="l"></i>loss</span>{/if}</span></div>
        {#each costs as r (r.account)}
          {@const whole = Math.max(r.value, r.cost ?? 0)}
          <div class="crow">
            <span class="cn">{r.name}</span>
            <span class="cb" style="width:{(whole / maxCost) * 100}%">
              {#if r.cost != null && r.gain != null}
                <!-- A gain extends past what you put in; a loss eats into it, shown in red at the end. -->
                <i class="c" style="width:{(Math.min(r.cost, r.value) / whole) * 100}%"></i>
                <i class={r.gain >= 0 ? "g" : "l"} style="width:{(Math.abs(r.gain) / whole) * 100}%"></i>
              {:else}<i class="c" style="width:100%;opacity:.5" title="some holdings have no book cost"></i>{/if}
            </span>
            <span class="cv num">
              {#if r.cost != null && r.gain != null}<Money value={r.cost} currency={cur} /> <Money value={r.gain} currency={cur} signed tone="delta" />{:else}<Money value={r.value} currency={cur} />{/if}
            </span>
          </div>
        {/each}
        {#if cashOnly.length && Math.abs(gap) >= 1}
          <p class="faint small gap">Home counts <Money value={gap} currency={cur} /> more in investments: {cashOnly.join(", ")} {cashOnly.length === 1 ? "holds" : "hold"} cash, not positions.</p>
        {/if}
      </section>
    {/if}

    <section class="card">
      <h2 class="label">Positions · grouped by security</h2>
      <table>
        <thead><tr><th>Symbol</th><th class="r">Units</th><th class="r">Price</th><th class="r">Value</th><th class="r">Weight</th><th class="r">Gain</th></tr></thead>
        <tbody>
          {#each p.securities as r (r.symbol)}
            {@const age = priceAge(r)}
            {@const multi = r.accounts.length > 1}
            <tr class="sec" class:multi onclick={() => multi && toggle(r.symbol)}>
              <td>
                <span class="sym">{r.symbol}</span>
                <span class="sub2">{r.accounts.map((a) => a.name).join(" + ")} · {r.currency}{#if r.currency !== cur && !r.missing_fx}&nbsp;→ {cur}{/if}{#if multi}&nbsp;<span class="tw">{open[r.symbol] ? "▾" : "▸"}</span>{/if}</span>
              </td>
              <td class="r num">{formatUnits(r.units)}</td>
              <td class="r num">
                {#if r.price == null}<span class="warn">no price</span><span class="sub2 warn">type one on Check-in</span>
                {:else}{formatPlain(r.price)}<span class="sub2" class:warn={age != null && age > STALE_DAYS}>{r.price_date ? formatDate(r.price_date) : ""}{r.price_source === "manual" ? " · manual" : ""}</span>{/if}
              </td>
              <td class="r">
                {#if r.missing_fx}<span class="warn" title="no FX rate for {r.currency}">no FX</span>
                {:else if r.value == null}<span class="faint">—</span>
                {:else}<Money value={r.value} currency={cur} />{/if}
              </td>
              <td class="r">
                {#if r.weight_pct != null}<span class="wt"><i style="width:{r.weight_pct}%"></i></span><span class="num">{formatPct(r.weight_pct)}</span>{:else}<span class="faint">—</span>{/if}
              </td>
              <td class="r">
                {#if r.gain != null}<Money value={r.gain} currency={cur} signed tone="delta" />{#if r.gain_pct != null}<span class="sub2">{Number(r.gain_pct) > 0 ? "+" : ""}{r.gain_pct}%</span>{/if}{:else}<span class="faint" title="add book_cost to the position to see gain">—</span>{/if}
              </td>
            </tr>
            {#if multi && open[r.symbol]}
              {#each r.accounts as a}
                <tr class="acct">
                  <td><a href="#/accounts/{encodeURIComponent(a.account)}">{a.name}</a></td>
                  <td class="r num">{formatUnits(a.units)}</td>
                  <td></td>
                  <td class="r">{#if a.value != null}<Money value={a.value} currency={cur} />{/if}</td>
                  <td></td>
                  <td class="r">{#if a.gain != null}<Money value={a.gain} currency={cur} signed tone="delta" />{/if}</td>
                </tr>
              {/each}
            {/if}
          {/each}
        </tbody>
      </table>
      <p class="faint note">
        Prices from {providerLabel(view.market.price_provider)}, FX from {providerLabel(view.market.fx_provider)}, fetched on startup and once a day; a price typed on Check-in wins for its date.
        Change sources under <code>settings.providers</code> in the ledger file. Fetched data lives in <code>quotes.json</code> beside it and can be deleted.
      </p>
    </section>
  {/if}
</div>

<style>
  .portfolio { display: flex; flex-direction: column; gap: 20px; }
  .error { color: var(--negative); }
  .top { display: flex; justify-content: space-between; align-items: center; gap: 12px; font-size: 12px; flex-wrap: wrap; }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; max-width: 56ch; }
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; }
  .chip { display: inline-flex; gap: 5px; align-items: baseline; }
  .warn { color: var(--warn); }
  .ch { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; margin-bottom: 6px; }
  .ch h2 { margin: 0; }
  .key { display: flex; gap: 14px; font-size: 12px; color: var(--text-2); }
  .key i, .cb i { display: inline-block; }
  .key i { width: 8px; height: 8px; border-radius: 2px; margin-right: 6px; }
  .crow { display: grid; grid-template-columns: 180px 1fr 210px; gap: 16px; align-items: center; padding: 7px 0; }
  .cn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cb { display: flex; gap: 2px; height: 12px; }
  .cb i { height: 100%; }
  .c { background: var(--fill); }
  .g { background: var(--positive); }
  .l { background: var(--negative); }
  .cv { text-align: right; display: flex; gap: 8px; justify-content: flex-end; }
  .gap { margin: 8px 0 0; }
  h2 { margin: 0 0 8px; font-weight: 500; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 12px; padding: 4px 0; }
  td { padding: 8px 0; border-top: 1px solid var(--border); vertical-align: middle; }
  .r { text-align: right; }
  .sym { font-weight: 500; }
  .sub2 { display: block; font-size: 11px; color: var(--text-3); }
  .multi { cursor: pointer; }
  .multi:hover .sym { color: var(--accent); }
  .tw { font-size: 10px; }
  .wt { display: inline-block; width: 64px; height: 6px; background: var(--surface-2); border-radius: 3px; vertical-align: middle; margin-right: 8px; overflow: hidden; }
  .wt i { display: block; height: 100%; background: var(--fill); }
  .acct td { padding: 6px 0 6px 18px; font-size: 12px; color: var(--text-2); border-top: 1px dashed var(--border); }
  .acct a { color: inherit; text-decoration: none; }
  .acct a:hover { color: var(--accent); }
  .note { font-size: 12px; margin: 12px 0 0; }
  code { font-family: var(--mono); font-size: 11px; }
  @media (max-width: 700px) {
    .crow { grid-template-columns: 1fr 120px; }
    .cb { grid-column: 1 / -1; grid-row: 2; }
    th:nth-child(2), td:nth-child(2), th:nth-child(5), td:nth-child(5) { display: none; }
    .wt { display: none; }
  }
</style>
