<script lang="ts">
  import PrivacyToggle from "../lib/components/PrivacyToggle.svelte";
  import type { LedgerView, SecurityRow } from "../lib/api";
  import { formatDate, formatPlain, formatUnits } from "../lib/money";
  import { providerLabel } from "../lib/market";
  import Money from "../lib/components/Money.svelte";
  import MarketLine from "../lib/components/MarketLine.svelte";
  import HoldingsLine from "../lib/components/HoldingsLine.svelte";
  import PortfolioTabs from "../lib/components/PortfolioTabs.svelte";
  import LookThrough from "./LookThrough.svelte";
  import Exposure from "./Exposure.svelte";
  import Overlap from "./Overlap.svelte";
  import { lookthrough } from "../lib/stores/lookthrough.svelte";
  import type { PortfolioTab } from "../lib/stores/route.svelte";

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
  // Allocation slices step down in tone; the text line beside them names each slice.
  const sliceFill = (i: number) => `color-mix(in oklab, var(--text) ${Math.max(50 - i * 11, 12)}%, var(--surface-2))`;

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

    <section class="card alloc">
      {#each [{ title: "By currency", slices: p.by_currency }, { title: "By account", slices: p.by_account }] as group}
        <div class="bar">
          <div class="lab"><span>{group.title}</span><span class="muted">{group.slices.map((s) => `${s.label} ${Math.round(Number(s.pct))}%`).join(" · ")}</span></div>
          <div class="track">
            {#each group.slices as s, i}
              {#if Number(s.pct) > 0}<i style="width:{s.pct}%;background:{sliceFill(i)}" title="{s.label} {s.pct}%"></i>{/if}
            {/each}
          </div>
        </div>
      {/each}
    </section>

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
                {#if r.weight_pct != null}<span class="wt"><i style="width:{r.weight_pct}%"></i></span><span class="num">{Number(r.weight_pct).toFixed(1)}%</span>{:else}<span class="faint">—</span>{/if}
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
  .alloc { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .bar .lab { display: flex; justify-content: space-between; gap: 10px; font-size: 12px; color: var(--text-2); margin-bottom: 6px; }
  .bar .lab .muted { text-align: right; }
  .track { height: 8px; border-radius: 4px; background: var(--surface-2); overflow: hidden; display: flex; gap: 2px; }
  .track i { display: block; height: 100%; }
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
    .alloc { grid-template-columns: 1fr; }
    th:nth-child(2), td:nth-child(2), th:nth-child(5), td:nth-child(5) { display: none; }
    .wt { display: none; }
  }
</style>
