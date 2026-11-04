<script lang="ts">
  import PrivacyToggle from "../../lib/components/shell/PrivacyToggle.svelte";
  import { KIND_LABEL, getAccountMonths, type LedgerView, type MonthSplit } from "../../lib/api";
  import { lastMonths } from "../../lib/networth/growth";
  import MonthBars from "../../lib/charts/MonthBars.svelte";
  import { formatDate, formatPlain, formatUnits, MASK, amountsHidden } from "../../lib/format/money";
  import { prefs } from "../../lib/stores/prefs.svelte";
  import Money from "../../lib/components/amounts/Money.svelte";
  import NetWorthChart from "../../lib/charts/NetWorthChart.svelte";

  let { view, id }: { view: LedgerView; id: string } = $props();
  const a = $derived(view.accounts.find((x) => x.id === id));
  const cur = $derived(view.base_currency);
  // Reuse the time-series chart: one account's value as the "net worth" series.
  const series = $derived((a?.history ?? []).map((h) => ({ date: h.date, net_worth: h.value, assets: h.value, liabilities: "0", by_group: {} })));
  const allRows = $derived([...(a?.history ?? [])].reverse());
  const SHOW = 24;
  let showAll = $state(false);
  const rows = $derived(showAll ? allRows : allRows.slice(0, SHOW));
  // Month by month, added vs grown; the full table of check-ins is one click away.
  let splits = $state<MonthSplit[]>([]);
  let key = "";
  $effect(() => {
    const k = `${id}|${view.version}`;
    if (k === key) return;
    key = k;
    getAccountMonths(id).then((m) => { splits = m; }, () => { splits = []; });
  });
  const months = $derived(lastMonths(splits, view.as_of));
  const sum = (f: (m: { added: number; grown: number }) => number) => months.reduce((s, m) => s + f(m), 0);
  let table = $state(false);
</script>

{#if !a}
  <p class="muted">No account called <code>{id}</code>. <a href="#/">Back to Home</a></p>
{:else}
  <div class="detail">
    <a class="back faint" href="#/">← Home</a>
    <div class="head">
      <div>
        <div class="label">{KIND_LABEL[a.kind]}{a.institution ? ` · ${a.institution}` : ""}{a.currency !== cur ? ` · ${a.currency}` : ""}</div>
        <h1>{a.name}</h1>
      </div>
      <div class="val">
        <Money value={a.value} currency={cur} size="xl" />
        <div class="faint"><PrivacyToggle /> {a.as_of ? `updated ${formatDate(a.as_of, "long")}` : "no value yet"}{a.days_stale ? ` · ${a.days_stale}d ago` : ""}</div>
      </div>
    </div>

    <section class="card">
      <NetWorthChart series={series} currency={cur} blur={prefs.blur} />
    </section>

    {#if a.holdings.length}
      <section class="card">
        <h2 class="label">Holdings</h2>
        <table class="hold">
          <thead><tr><th>Symbol</th><th class="r">Units</th><th class="r">Price ({a.currency})</th><th class="r">Value ({a.currency})</th><th class="r">Book cost</th><th class="r">Gain</th></tr></thead>
          <tbody>
            {#each a.holdings as h}
              <tr>
                <td>{h.symbol}</td>
                <td class="r num">{formatUnits(h.units)}</td>
                <td class="r num">{#if h.price == null}<span class="warn">no price</span>{:else}{formatPlain(h.price)}<span class="faint small">&nbsp;&nbsp;{h.price_date ? formatDate(h.price_date) : ""}{h.price_source === "manual" ? " · manual" : ""}</span>{/if}</td>
                <td class="r num money">{h.native_value == null ? "—" : amountsHidden() ? MASK : formatPlain(h.native_value)}</td>
                <td class="r num money">{h.book_cost == null ? "—" : amountsHidden() ? MASK : formatPlain(h.book_cost)}</td>
                <td class="r">{#if h.native_value != null && h.book_cost != null}<Money value={Number(h.native_value) - Number(h.book_cost)} currency={a.currency} signed tone="delta" />{:else}—{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}

    <section class="card">
      <div class="hh"><h2 class="label">Month by month</h2><span class="faint small">last 12 months{#if a.liability}&nbsp;· paid down = added{/if}</span></div>
      {#if rows.length === 0}
        <p class="muted">No values recorded yet. <a href="#/check-in">Check in</a> to add the first one.</p>
      {:else}
        {#if splits.length}
          <MonthBars {months} currency={cur} blur={prefs.blur} />
          <div class="key">
            <span><i class="ka"></i>You added <b><Money value={sum((m) => m.added)} currency={cur} signed /></b></span>
            <span><i class="kg"></i>{a.group === "property" ? "Value change" : "Market and FX"} <b><Money value={sum((m) => m.grown)} currency={cur} signed /></b></span>
            <button type="button" class="link" aria-expanded={table} onclick={() => (table = !table)}>{table ? "Hide every check-in" : `Every check-in (${allRows.length})`}</button>
          </div>
        {/if}
        {#if table || !splits.length}
        <table>
          <thead><tr><th>Date</th><th class="r">Value ({cur})</th><th class="r">Change</th></tr></thead>
          <tbody>
            {#each rows as h, i}
              <tr>
                <td>{formatDate(h.date, "long")}</td>
                <td class="r"><Money value={h.value} currency={cur} digits={2} /></td>
                <td class="r">{#if i < allRows.length - 1}<Money value={Number(h.value) - Number(allRows[i + 1].value)} currency={cur} signed tone={a.liability ? "none" : "delta"} />{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        {#if allRows.length > SHOW}
          <button type="button" class="link" onclick={() => (showAll = !showAll)}>{showAll ? "Show recent only" : `Show all ${allRows.length} rows`}</button>
        {/if}
        {/if}
      {/if}
    </section>
  </div>
{/if}

<style>
  .detail { display: flex; flex-direction: column; gap: 18px; }
  .back { text-decoration: none; font-size: 12px; }
  .head { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; flex-wrap: wrap; }
  h1 { margin: 2px 0 0; font-size: 22px; font-weight: 600; letter-spacing: -0.01em; }
  .val { text-align: right; }
  h2 { margin: 0 0 8px; font-weight: 500; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 12px; padding: 4px 0; }
  td { padding: 7px 0; border-top: 1px solid var(--border); }
  .r { text-align: right; padding-left: 10px; }
  .small { font-size: 11px; }
  /* Holdings on a phone: units and book cost step aside so value and gain stay on screen. */
  @media (max-width: 600px) {
    .hold th:nth-child(2), .hold td:nth-child(2), .hold th:nth-child(5), .hold td:nth-child(5) { display: none; }
  }
  .warn { color: var(--warn); }
  .hh { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  .key { display: flex; flex-wrap: wrap; gap: 6px 18px; align-items: baseline; font-size: 12px; color: var(--text-2); margin-top: 8px; }
  .key i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 6px; }
  .key b { color: var(--text); font-weight: 500; }
  .ka { background: var(--fill); }
  .kg { background: repeating-linear-gradient(-45deg, var(--accent) 0 2px, transparent 2px 4px); box-shadow: inset 0 0 0 1px var(--accent); }
  .key .link { margin-left: auto; padding: 0; }
  .link { color: var(--accent); background: none; border: 0; font: inherit; cursor: pointer; font-size: 12px; padding: 10px 0 0; }
  code { font-family: var(--mono); }
</style>
