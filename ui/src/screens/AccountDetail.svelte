<script lang="ts">
  import { KIND_LABEL, type LedgerView } from "../lib/api";
  import { formatDate, formatPlain } from "../lib/money";
  import { prefs } from "../lib/stores/prefs.svelte";
  import Money from "../lib/components/Money.svelte";
  import NetWorthChart from "../lib/charts/NetWorthChart.svelte";

  let { view, id }: { view: LedgerView; id: string } = $props();
  const a = $derived(view.accounts.find((x) => x.id === id));
  const cur = $derived(view.base_currency);
  // Reuse the time-series chart: one account's value as the "net worth" series.
  const series = $derived((a?.history ?? []).map((h) => ({ date: h.date, net_worth: h.value, assets: h.value, liabilities: "0", by_group: {} })));
  const rows = $derived([...(a?.history ?? [])].reverse());
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
        <div class="faint">{a.as_of ? `updated ${formatDate(a.as_of, "long")}` : "no value yet"}{a.days_stale != null ? ` · ${a.days_stale}d ago` : ""}</div>
      </div>
    </div>

    <section class="card">
      <NetWorthChart series={series} currency={cur} blur={prefs.blur} groups={false} />
    </section>

    {#if a.holdings.length}
      <section class="card">
        <h2 class="label">Holdings</h2>
        <table>
          <thead><tr><th>Symbol</th><th class="r">Units</th><th class="r">Price</th><th class="r">Value</th><th class="r">Book cost</th></tr></thead>
          <tbody>
            {#each a.holdings as h}
              <tr>
                <td>{h.symbol}</td>
                <td class="r num">{formatPlain(h.units, 4)}</td>
                <td class="r num">{h.price == null ? "—" : formatPlain(h.price)}<span class="faint small">&nbsp;&nbsp;{h.price_date ? formatDate(h.price_date) : ""}</span></td>
                <td class="r num money">{h.price == null ? "—" : formatPlain(Number(h.units) * Number(h.price))}</td>
                <td class="r num money">{h.book_cost == null ? "—" : formatPlain(h.book_cost)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}

    <section class="card">
      <h2 class="label">History</h2>
      {#if rows.length === 0}
        <p class="muted">No values recorded yet. <a href="#/check-in">Check in</a> to add the first one.</p>
      {:else}
        <table>
          <thead><tr><th>Date</th><th class="r">Value ({cur})</th><th class="r">Change</th></tr></thead>
          <tbody>
            {#each rows as h, i}
              <tr>
                <td>{formatDate(h.date, "long")}</td>
                <td class="r"><Money value={h.value} currency={cur} digits={2} /></td>
                <td class="r">{#if i < rows.length - 1}<Money value={Number(h.value) - Number(rows[i + 1].value)} currency={cur} signed tone={a.liability ? "none" : "delta"} />{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table>
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
  th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; padding: 4px 0; }
  td { padding: 7px 0; border-top: 1px solid var(--border); }
  .r { text-align: right; }
  .small { font-size: 11px; }
  code { font-family: var(--mono); }
</style>
