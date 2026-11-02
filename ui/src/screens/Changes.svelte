<script lang="ts">
  // Changes: what moved between two dates. Net worth bridge, every account, every
  // holding split into market / trades / FX, exposure drift, and the feed of fund
  // file changes. All figures come from GET /api/changes; this file lays them out.
  import { GROUPS, GROUP_LABEL, type AccountDiff, type BucketDelta, type Drift, type LedgerView } from "../lib/api";
  import { formatDate, formatPlain, formatSigned } from "../lib/money";
  import { PRESETS, accountHint, bridgeSteps, changeLine, minorLine, parts, presetFrom, presetTo, rangeLabel, type Preset } from "../lib/changes";
  import { changes as store } from "../lib/stores/changes.svelte";
  import { prefs } from "../lib/stores/prefs.svelte";
  import Money from "../lib/components/Money.svelte";
  import MarketLine from "../lib/components/MarketLine.svelte";
  import Bridge from "../lib/charts/Bridge.svelte";
  import SplitStrip from "../lib/components/SplitStrip.svelte";
  import DeltaBar from "../lib/components/DeltaBar.svelte";
  import WeightMove from "../lib/components/WeightMove.svelte";

  let { view }: { view: LedgerView } = $props();
  const cur = $derived(view.base_currency);
  const today = $derived(view.as_of);
  // The period: a preset or two typed dates; the server fills in "since last check-in".
  const from = $derived(presetFrom(store.preset, today, store.current?.first_date ?? null, store.custom));
  const to = $derived(presetTo(store.preset, today, store.custom));
  $effect(() => {
    void store.ensure(view.version, from, to);
  });
  const v = $derived(store.current);
  const c = $derived(v?.changes);
  let customFrom = $state("");
  let customTo = $state("");
  function pick(p: Preset) {
    if (p === "custom") {
      customFrom = customFrom || v?.from || today;
      customTo = customTo || v?.to || today;
      store.setCustom(customFrom, customTo);
    } else {
      store.setPreset(p);
    }
  }
  function applyCustom() {
    if (customFrom && customTo && customFrom <= customTo) store.setCustom(customFrom, customTo);
  }

  const headParts = $derived(c ? parts(c) : []);
  // Column and bridge labels carry the year only when the two ends differ in it.
  const endLabel = (d: string) => formatDate(d, c && c.from.slice(0, 4) !== c.to.slice(0, 4) ? "long" : "short");
  const steps = $derived(c ? bridgeSteps(c.accounts) : []);
  const byGroup = $derived(
    c ? GROUPS.map((g) => ({ g, rows: c.accounts.filter((a) => a.group === g) })).filter((x) => x.rows.length) : [],
  );
  /** "bought 14 XEQT.TO" notes for one account, from the ledger's trade log. */
  function tradeNotes(a: AccountDiff): string[] {
    if (!c) return [];
    const acct = view.accounts.find((x) => x.id === a.account);
    if (!acct) return [];
    return c.holdings
      .filter((h) => h.accounts.includes(acct.name) && h.trade_count > 0)
      .map((h) => {
        const d = Number(h.to_units) - Number(h.from_units);
        return `${d >= 0 ? "bought" : "sold"} ${formatPlain(Math.abs(d), Number.isInteger(d) ? 0 : 2)} ${h.symbol}`;
      });
  }

  const units = (x: string) => new Intl.NumberFormat("en-CA", { maximumFractionDigits: 4 }).format(Number(x));
  const accountParts = (a: AccountDiff) =>
    [
      { label: "market", value: Number(a.market ?? 0) },
      { label: "trades", value: Number(a.trades ?? 0) },
      { label: "balance", value: Number(a.balance ?? 0) },
      { label: "fx", value: Number(a.fx ?? 0) },
      { label: "unpriced", value: Number(a.residual) },
    ];
  const maxAccount = $derived(Math.max(0, ...(c?.accounts.map((a) => Math.abs(Number(a.change))) ?? [])));
  const maxHolding = $derived(Math.max(0, ...(c?.holdings.map((h) => Math.abs(Number(h.change ?? 0))) ?? [])));
  const HOLD_SHOW = 12;
  let allHoldings = $state(false);
  const holdings = $derived(c ? (allHoldings ? c.holdings : c.holdings.slice(0, HOLD_SHOW)) : []);

  let mode = $state<"lookthrough" | "as_held">("lookthrough");
  // Look-through drift needs a file at both ends; with none old enough for the start, as held is the honest view.
  const anyFiles = $derived((v?.funds_resolved_to ?? 0) > 0 && (v?.funds_resolved_from ?? 0) > 0);
  const noOldFiles = $derived((v?.funds_resolved_to ?? 0) > 0 && (v?.funds_resolved_from ?? 0) === 0);
  const eff = $derived<"lookthrough" | "as_held">(anyFiles ? mode : "as_held");
  const drift = $derived<Drift | null>(v ? (eff === "lookthrough" ? v.drift : v.as_held_drift) : null);
  /** Rows that moved, biggest move first, capped so each set stays short. */
  function moved(b: BucketDelta[], n = 8): BucketDelta[] {
    return b.filter((x) => Number(x.delta) !== 0).sort((p, q) => Math.abs(Number(q.delta)) - Math.abs(Number(p.delta))).slice(0, n);
  }
  const driftSets = $derived(
    drift
      ? [
          { title: "Sector", rows: moved(drift.by_sector) },
          { title: "Region", rows: moved(drift.by_region) },
          { title: "Country", rows: moved(drift.by_country) },
          { title: eff === "lookthrough" ? "Currency of the underlying" : "Currency traded in", rows: moved(drift.by_currency, 6) },
        ].filter((s) => s.rows.length)
      : [],
  );
  const maxPts = $derived(Math.max(0.1, ...driftSets.flatMap((s) => s.rows.map((r) => Math.abs(Number(r.delta))))));

  const FEED_SHOW = 6;
  let allFunds = $state(false);
  const feed = $derived(v ? (allFunds ? v.funds : v.funds.slice(0, FEED_SHOW)) : []);
  const THRESHOLDS = ["0.1", "0.5", "1.0"];
</script>

<div class="changes">
  <div class="top faint"><span>as of {formatDate(view.as_of, "long")} · {cur}</span>{#if view.accounts.length}<MarketLine {view} />{/if}</div>

  {#if view.accounts.length === 0}
    <section class="card empty">
      <h2>Nothing to compare yet</h2>
      <p class="muted">Changes shows what moved between two check-ins. Add accounts and enter balances on <a href="#/check-in">Check-in</a>; the second check-in gives this screen something to say.</p>
    </section>
  {:else}
    <div class="range">
      <div class="seg" role="radiogroup" aria-label="Period">
        {#each PRESETS as p (p.id)}
          <button type="button" role="radio" aria-checked={store.preset === p.id} class:on={store.preset === p.id} title={p.title} onclick={() => pick(p.id)}>{p.label}</button>
        {/each}
      </div>
      {#if store.preset === "custom"}
        <span class="custom">
          <input type="date" aria-label="From" bind:value={customFrom} max={customTo || today} onchange={applyCustom} />
          <span class="faint">→</span>
          <input type="date" aria-label="To" bind:value={customTo} min={customFrom} max={today} onchange={applyCustom} />
        </span>
      {/if}
      {#if c}<span class="dates muted">{rangeLabel(c.from, c.to, c.days)}</span>{/if}
    </div>

    {#if store.error && !v}
      <p class="error">Could not load changes: {store.error}</p>
    {:else if !v || !c}
      <p class="faint">Loading…</p>
    {:else if c.accounts.length === 0 || c.days === 0}
      <section class="card empty">
        <h2>One check-in so far</h2>
        <p class="muted">Nothing was recorded before {formatDate(c.to, "long")}. After the next check-in this screen shows what moved, account by account.</p>
      </section>
    {:else}
      <section class="hero">
        <div class="label">Net worth change</div>
        <Money value={c.change} currency={cur} size="xl" signed tone="delta" />
        <div class="sub">
          <span class="chip"><span class="num"><Money value={c.from_net_worth} currency={cur} /> → <Money value={c.to_net_worth} currency={cur} /></span>{#if c.pct != null}<span class="muted">({Number(c.pct) > 0 ? "+" : ""}{c.pct}%)</span>{/if}</span>
        </div>
        {#if headParts.length}
          <div class="split"><SplitStrip parts={headParts} currency={cur} height={10} legend blur={prefs.blur} /></div>
        {/if}
      </section>

      {#if steps.length}
        <section class="card">
          <h2 class="label">What moved it</h2>
          <div class="bridge-wrap"><Bridge start={Number(c.from_net_worth)} end={Number(c.to_net_worth)} {steps} startLabel={endLabel(c.from)} endLabel={endLabel(c.to)} currency={cur} blur={prefs.blur} /></div>
          <p class="faint note">Debt paid down counts as a gain.{#if steps.some((s) => s.folded)}&nbsp;Smallest movers folded into the last step.{/if}</p>
        </section>
      {/if}

      <section class="card">
        <div class="head"><h2 class="label">Accounts</h2><span class="faint small">{c.accounts.length} {c.accounts.length === 1 ? "account" : "accounts"} · biggest move first</span></div>
        <table class="accts">
          <thead><tr><th>Account</th><th class="r">{endLabel(c.from)}</th><th class="r">{endLabel(c.to)}</th><th class="bar"></th><th class="r">Change</th><th class="r">%</th><th class="r why" title="What a mixed change is made of: market, trades, balance, FX; hover the row">Why</th></tr></thead>
          <tbody>
            {#each byGroup as { g, rows } (g)}
              <tr class="grp"><td colspan="7">{GROUP_LABEL[g]}{#if g === "debt"}<span class="faint"> · paid down = gain</span>{/if}</td></tr>
              {#each rows as a (a.account)}
                {@const hint = accountHint(a, cur, tradeNotes(a))}
                <tr title={hint}>
                  <td><a class="name" href="#/accounts/{encodeURIComponent(a.account)}">{a.name}</a>{#if a.from_value == null}<span class="tag">new</span>{/if}{#if a.missing_fx || a.missing_price}<span class="tag warn">{a.missing_fx ? "no FX" : "no price"}</span>{/if}</td>
                  <td class="r">{#if a.from_value == null}<span class="faint">—</span>{:else}<Money value={a.from_value} currency={cur} />{/if}</td>
                  <td class="r">{#if a.to_value == null}<span class="faint">—</span>{:else}<Money value={a.to_value} currency={cur} />{/if}</td>
                  <td class="bar"><DeltaBar value={Number(a.change)} max={maxAccount} /></td>
                  <td class="r">{#if Number(a.change) === 0}<span class="faint">—</span>{:else}<Money value={a.change} currency={cur} signed tone="delta" />{/if}</td>
                  <td class="r num" class:faint={a.pct == null || Number(a.pct) === 0}>{a.pct == null ? "—" : `${Number(a.pct) > 0 ? "+" : ""}${a.pct}%`}</td>
                  <td class="r why">{#if accountParts(a).filter((p) => Math.abs(p.value) >= 0.005).length > 1}<SplitStrip parts={accountParts(a)} currency={cur} width="56px" height={6} />{/if}</td>
                </tr>
              {/each}
            {/each}
          </tbody>
        </table>
      </section>

      {#if c.holdings.length}
        <section class="card">
          <div class="head"><h2 class="label">Holdings</h2><span class="faint small" title="Market: the price move on units already held. Trades: units bought or sold, at the end price. FX: the rate move on the starting value. The three add up to the change exactly.">change = market + trades + FX · hover a row</span></div>
          <table class="hold">
            <thead><tr><th>Symbol</th><th class="r">Units</th><th class="r">Price</th><th class="bar"></th><th class="r">Change</th><th class="r why" title="market · trades · FX, left to right">Why</th></tr></thead>
            <tbody>
              {#each holdings as h (h.symbol)}
                {@const hp = [{ label: "market", value: Number(h.market ?? 0) }, { label: "trades", value: Number(h.trades ?? 0) }, { label: "fx", value: Number(h.fx ?? 0) }]}
                <tr title={hp.filter((p) => p.value !== 0).map((p) => `${p.label} ${formatSigned(p.value, cur)}`).join(" · ") || undefined}>
                  <td><span class="sym">{h.symbol}</span><span class="sub2">{h.accounts.join(" + ")}{#if h.currency !== cur}&nbsp;· {h.currency}{/if}{#if h.trade_count}&nbsp;· {h.trade_count} {h.trade_count === 1 ? "trade" : "trades"}{/if}</span></td>
                  <td class="r num">{#if Number(h.from_units) !== Number(h.to_units)}<span class="faint">{units(h.from_units)} →</span> {/if}{units(h.to_units)}</td>
                  <td class="r num">
                    {#if h.to_price == null}<span class="warn">no price</span>
                    {:else}{#if h.from_price != null && h.from_price !== h.to_price}<span class="faint">{formatPlain(h.from_price)} →</span> {/if}{formatPlain(h.to_price)}{/if}
                  </td>
                  <td class="bar"><DeltaBar value={Number(h.change ?? 0)} max={maxHolding} /></td>
                  <td class="r">{#if h.change == null}<span class="warn" title="no price at one end of the period">unpriced</span>{:else}<Money value={h.change} currency={cur} signed tone="delta" />{/if}</td>
                  <td class="r why">{#if hp.filter((p) => Math.abs(p.value) >= 0.005).length > 1}<SplitStrip parts={hp} currency={cur} width="56px" height={6} />{/if}</td>
                </tr>
              {/each}
            </tbody>
          </table>
          {#if c.holdings.length > HOLD_SHOW}
            <button type="button" class="more" onclick={() => (allHoldings = !allHoldings)}>{allHoldings ? "Show fewer" : `Show ${c.holdings.length - HOLD_SHOW} more`}</button>
          {/if}
        </section>
      {/if}

      {#if drift && c.holdings.length}
        <section class="card">
          <div class="head">
            <h2 class="label">Exposure drift{#if drift && eff === "lookthrough" && drift.from_as_of && drift.to_as_of}<span class="faint sub-h"> · holdings files {formatDate(drift.from_as_of)} → {formatDate(drift.to_as_of)}</span>{/if}</h2>
            <div class="toggle" role="radiogroup" aria-label="View">
              <button type="button" role="radio" aria-checked={eff === "lookthrough"} class:on={eff === "lookthrough"} onclick={() => (mode = "lookthrough")} disabled={!anyFiles} title={anyFiles ? "" : noOldFiles ? `No holdings file old enough for ${v ? formatDate(v.from) : "the start"}` : "No holdings files yet"}>Look-through</button>
              <button type="button" role="radio" aria-checked={eff === "as_held"} class:on={eff === "as_held"} onclick={() => (mode = "as_held")}>As held</button>
            </div>
          </div>
          {#if driftSets.length === 0}
            <p class="muted empty-line">No bucket moved a tenth of a point{#if eff === "lookthrough"}&nbsp;through funds{/if} in this period.</p>
          {/if}
          <div class="two">
            {#each driftSets as s (s.title)}
              <div class="set">
                <div class="label">{s.title}</div>
                {#each s.rows as r (r.label)}
                  {@const d = Number(r.delta)}
                  <div class="drow" title="{r.label}: {Number(r.from_pct).toFixed(1)}% → {Number(r.to_pct).toFixed(1)}%">
                    <span class="name">{r.label}</span>
                    <span class="div"><i class:pos={d > 0} class:neg={d < 0} style="width:{(Math.abs(d) / maxPts) * 50}%"></i></span>
                    <span class="pct num">{Number(r.from_pct).toFixed(1)}<span class="faint">→</span>{Number(r.to_pct).toFixed(1)}</span>
                    <span class="pts num" class:up={d > 0} class:down={d < 0} class:faint={d === 0}>{d > 0 ? "+" : d < 0 ? "−" : ""}{Math.abs(d).toFixed(1)}</span>
                  </div>
                {/each}
              </div>
            {/each}
          </div>
          <p class="faint note">{#if noOldFiles}No holdings file is old enough for {formatDate(v.from)}, so this is as held. {/if}Percentage points of your investments{#if eff === "lookthrough"}, through every fund{/if}.</p>
        </section>
      {/if}

      <section class="card">
        <div class="head">
          <h2 class="label">Fund changes</h2>
          <span class="faint small thr">moves of
            <span class="seg mini" role="radiogroup" aria-label="Threshold">
              {#each THRESHOLDS as t}<button type="button" role="radio" aria-checked={store.threshold === t} class:on={store.threshold === t} onclick={() => store.setThreshold(t)}>{t}</button>{/each}
            </span> pt or more</span>
        </div>
        {#if v.funds.length === 0}
          <p class="muted empty-line">
            {#if v.funds_resolved_to === 0}No holdings files yet, so nothing to compare. Files arrive on <a href="#/portfolio/look-through">Look-through</a>.
            {:else if v.same_file.length && !v.no_earlier_file.length}No fund published a new holdings file in this period{#if v.drift.to_as_of}&nbsp;(latest {formatDate(v.drift.to_as_of)}){/if}. Pick a longer period to compare files.
            {:else}Only one holdings file per fund so far; the first comparison appears once a newer file is fetched.{/if}
          </p>
        {:else}
          <div class="feed">
            {#each feed as f (f.symbol)}
              <div class="item">
                <span class="who"><span class="sym">{f.symbol}</span><span class="when">{formatDate(f.from)} → {formatDate(f.to)}{#if !f.held}&nbsp;· inside a fund you hold{/if}</span></span>
                <span class="what">
                  {#if f.changes.length === 0}
                    <span class="muted">Nothing moved {Number(f.threshold).toFixed(1)} pt or more</span>
                  {:else}
                    {@const maxW = Math.max(...f.changes.slice(0, 3).flatMap((ch) => [Number(ch.from_weight), Number(ch.to_weight)]), 0.1)}
                    {#each f.changes.slice(0, 3) as ch (ch.key)}
                      {@const d = Number(ch.delta)}
                      <span class="line" title={changeLine(ch)}>
                        <span class="tag" class:up={ch.kind === "added"} class:down={ch.kind === "removed"}>{ch.kind === "reweighted" ? (d > 0 ? "up" : "down") : ch.kind}</span>
                        <span class="cname">{ch.ticker && ch.ticker !== "-" ? ch.ticker : ch.name}</span>
                        <WeightMove from={Number(ch.from_weight)} to={Number(ch.to_weight)} max={maxW} />
                        <span class="num wnum">{Number(ch.from_weight).toFixed(2)}<span class="faint">→</span>{Number(ch.to_weight).toFixed(2)}%</span>
                        <span class="num pts" class:up={d > 0} class:down={d < 0}>{d > 0 ? "+" : "−"}{Math.abs(d).toFixed(2)}</span>
                        {#if ch.your_value != null && Number(ch.your_value) !== 0}<span class="eff"><Money value={ch.your_value} currency={cur} signed tone="delta" /></span>{/if}
                      </span>
                    {/each}
                    {#if f.changes.length > 3}<span class="sub2">+{f.changes.length - 3 + f.hidden} more</span>{/if}
                  {/if}
                  <span class="sub2" title={minorLine(f)}>{f.to_constituents.toLocaleString("en-CA")} names · {Number(f.turnover_pct).toFixed(1)}% turnover{#if f.your_value != null}&nbsp;· <Money value={f.your_value} currency={cur} size="s" /> of yours{/if}</span>
                </span>
              </div>
            {/each}
          </div>
          {#if v.funds.length > FEED_SHOW}
            <button type="button" class="more" onclick={() => (allFunds = !allFunds)}>{allFunds ? "Show fewer" : `Show ${v.funds.length - FEED_SHOW} more funds`}</button>
          {/if}
          <p class="faint note" title="Each line compares the fund's holdings file in effect at the start of the period with the one at the end. The right-hand dollars are your money in that fund × the weight change.">{#if v.same_file.length}{v.same_file.length} {v.same_file.length === 1 ? "fund" : "funds"} had no new file this period ({v.same_file.join(", ")}). {/if}Mark = old weight, bar = move; hover for words.</p>
        {/if}
      </section>
    {/if}
  {/if}
</div>

<style>
  .changes { display: flex; flex-direction: column; gap: 20px; }
  .top { display: flex; justify-content: space-between; align-items: center; gap: 12px; font-size: 12px; flex-wrap: wrap; }
  .error { color: var(--negative); }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; max-width: 56ch; }
  .range { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; font-size: 12px; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
  .seg button { padding: 5px 11px; font: inherit; font-size: 12px; color: var(--text-2); background: none; border: 0; cursor: pointer; }
  .seg button.on { background: var(--accent-soft); color: var(--text); }
  .seg button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .seg.mini { vertical-align: middle; margin: 0 2px; }
  .seg.mini button { padding: 1px 7px; font-size: 11px; font-family: var(--figures); }
  .custom { display: inline-flex; align-items: center; gap: 6px; }
  .custom input { font: inherit; font-size: 12px; color: var(--text); background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 3px 6px; color-scheme: inherit; }
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; }
  .split { margin-top: 12px; max-width: 560px; }
  th.bar, td.bar { width: 80px; padding-left: 10px; padding-right: 10px; }
  th.why, td.why { width: 64px; }
  .tag { font-size: 10px; text-transform: uppercase; letter-spacing: 0.08em; padding: 1px 6px; border-radius: 4px; background: var(--surface-2); color: var(--text-2); margin-left: 6px; }
  .tag.warn { color: var(--warn); }
  .tag.up { color: var(--positive); }
  .tag.down { color: var(--negative); }
  .cname { min-width: 56px; }
  .wnum { font-size: 11px; color: var(--text-2); }
  .pts { width: 44px; text-align: right; }
  .chip { display: inline-flex; gap: 5px; align-items: baseline; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  .small { font-size: 12px; }
  .sub-h { text-transform: none; letter-spacing: 0; }
  h2 { margin: 0 0 8px; font-weight: 500; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; padding: 4px 0; }
  td { padding: 7px 0; border-top: 1px solid var(--border); vertical-align: middle; }
  .r { text-align: right; }
  .grp td { border-top: 0; padding: 12px 0 2px; font-size: 11px; text-transform: uppercase; letter-spacing: 0.1em; color: var(--text-3); }
  .grp td .faint { text-transform: none; letter-spacing: 0; }
  .name { color: inherit; text-decoration: none; }
  .name:hover { color: var(--accent); }
  .sym { font-weight: 500; }
  .sub2 { display: block; font-size: 11px; color: var(--text-3); }
  .warn { color: var(--warn); }
  .note { font-size: 12px; margin: 10px 0 0; }
  .more { color: var(--accent); background: none; border: 0; font: inherit; cursor: pointer; font-size: 12px; padding: 10px 0 0; }
  .toggle { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
  .toggle button { padding: 4px 10px; font: inherit; font-size: 12px; color: var(--text-2); background: none; border: 0; cursor: pointer; }
  .toggle button.on { background: var(--accent-soft); color: var(--text); }
  .toggle button:disabled { opacity: 0.5; cursor: default; }
  .bridge-wrap { overflow-x: auto; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 18px 24px; margin-top: 6px; }
  .set .label { margin-bottom: 6px; }
  .drow { display: grid; grid-template-columns: minmax(96px, 130px) 1fr 76px 42px; gap: 10px; align-items: center; font-size: 12px; padding: 3px 0; }
  .drow .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .div { position: relative; height: 10px; background: var(--surface-2); border-radius: 3px; overflow: hidden; }
  .div::before { content: ""; position: absolute; left: 50%; top: 0; bottom: 0; width: 1px; background: var(--border); }
  .div i { position: absolute; top: 0; bottom: 0; min-width: 1px; }
  .div i.pos { left: 50%; background: var(--positive); border-radius: 0 3px 3px 0; }
  .div i.neg { right: 50%; background: var(--negative); border-radius: 3px 0 0 3px; }
  .pct { text-align: right; color: var(--text-2); }
  .pct .faint { margin: 0 2px; }
  .pts { text-align: right; }
  .up { color: var(--positive); }
  .down { color: var(--negative); }
  .thr { display: inline-flex; align-items: center; gap: 4px; }
  .empty-line { margin: 4px 0 0; font-size: 13px; }
  .feed { display: flex; flex-direction: column; }
  .item { display: grid; grid-template-columns: 128px 1fr; gap: 14px; padding: 9px 0; border-top: 1px solid var(--border); font-size: 13px; }
  .item:first-of-type { border-top: 0; }
  .who { display: flex; flex-direction: column; }
  .when { font-size: 11px; color: var(--text-3); }
  .what { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .line { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .eff { font-size: 12px; color: var(--text-2); margin-left: auto; white-space: nowrap; }
  .line .tag { margin-left: 0; width: 58px; text-align: center; }
  @media (max-width: 760px) {
    .two { grid-template-columns: 1fr; }
    .hold .bar, .hold .why, .accts .bar, .accts .why { display: none; }
    .wnum { display: none; }
    .item { grid-template-columns: 1fr; gap: 4px; }
  }
</style>
