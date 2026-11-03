<script lang="ts">
  // One fund or company: its price, what you have in it over time, and what it is made of
  // (a fund) or which of your funds carry it (a company). Reached from any fund or company
  // name on Portfolio, Changes and ⌘K.
  import { getSecurity, errorMessage, type Company, type LedgerView, type SecurityView } from "../../lib/api";
  import { formatDate, formatMoney, formatPct, formatPlain, formatUnits } from "../../lib/format/money";
  import { closestTwin, familyHue, familyOf, fundMix, overlapPairs, viaLabel, FLAG } from "../../lib/portfolio/lookthrough";
  import { RANGES, insideOf, quoteSymbol, window, type Range } from "../../lib/portfolio/security";
  import { lookthrough } from "../../lib/stores/portfolio/lookthrough.svelte";
  import { hashFor } from "../../lib/stores/route.svelte";
  import Money from "../../lib/components/amounts/Money.svelte";
  import PrivacyToggle from "../../lib/components/shell/PrivacyToggle.svelte";
  import Lineage from "../../lib/components/portfolio/Lineage.svelte";
  import LineChart from "../../lib/charts/LineChart.svelte";

  let { view, symbol, company }: { view: LedgerView; symbol?: string; company?: string } = $props();
  $effect(() => { void lookthrough.ensure(view.version); });
  const lt = $derived(lookthrough.current);
  const cur = $derived(view.base_currency);
  const x = $derived(lt?.lookthrough);
  const co = $derived<Company | null>(company && x ? x.companies.find((c) => c.key === company) ?? null : null);
  const sym = $derived(company ? (co ? quoteSymbol(co) : null) : symbol ?? null);
  const held = $derived(sym ? view.portfolio.securities.find((r) => r.symbol === sym) ?? null : null);
  const fund = $derived(sym && x ? x.funds.find((f) => f.symbol === sym) ?? null : null);
  const snapInfo = $derived(sym && lt ? lt.snapshots.find((s) => s.symbol === sym) ?? null : null);
  const isFund = $derived(!company && (!!fund || !!snapInfo));
  const name = $derived(co?.name ?? fund?.name ?? snapInfo?.name ?? null);
  // A fund inside one of yours (XUS.TO inside XEQT.TO): the funds that lead to it.
  const parents = $derived(!company && sym && x && !held ? [...new Set(x.companies.flatMap((c) => c.via.filter((v) => v.path.includes(sym) && v.path[0] !== sym).map((v) => v.path.slice(0, v.path.indexOf(sym)).join(" → "))))] : []);
  const inside = $derived(isFund && sym && x ? insideOf(x.companies, sym, 12) : []);
  const mix = $derived(isFund && sym && x ? fundMix(x.companies, sym) : []);
  const pairs = $derived(lt ? overlapPairs(lt.overlap) : []);
  const twins = $derived(sym ? pairs.filter((p) => (p.a === sym || p.b === sym)) : []);
  const invTotal = $derived(Number(view.portfolio.total) + view.accounts.filter((a) => a.group === "investments" && a.positions == null).reduce((s, a) => s + Number(a.value ?? 0), 0));

  let data = $state<SecurityView | null>(null);
  let error = $state<string | null>(null);
  $effect(() => {
    const s = sym, k = company, v = view.version;
    void v;
    if (!s) return;
    data = null; error = null;
    getSecurity(s, k).then((d) => { if (s === sym) data = d; }).catch((e) => { error = errorMessage(e); });
  });
  const yoursNow = $derived(data?.yours.length ? Number(data.yours[data.yours.length - 1].value) : co ? Number(co.value) : held?.value != null ? Number(held.value) : 0);
  let range = $state<Range>("1y");
  let show = $state<"price" | "yours">("price");
  const win = $derived(data ? window(data.prices, range) : null);
  const mode = $derived(win ? show : "yours");
  const yoursPts = $derived((data?.yours ?? []).map((y) => ({ date: y.date, v: Number(y.value) })));
  const famHue = (slot: string) => { const h = familyHue(familyOf(slot)); return h === "--f-none" ? "var(--fill)" : `var(${h})`; };
</script>

<div class="sec">
  <a class="back faint" href={hashFor({ name: "portfolio", tab: "lookthrough" })}>‹ Portfolio</a>

  {#if company && lt && !co}
    <section class="card empty"><h2>Not in your funds</h2><p class="muted">None of your funds holds this company in the files on hand.</p></section>
  {:else if !sym && !lt}
    <p class="faint">Loading…</p>
  {:else}
    <header class="head">
      <h1>{co ? co.name : sym}</h1>
      {#if co && sym}<span class="tk faint">{sym}</span>{:else if name}<span class="nm muted">{name}</span>{/if}
      <div class="tags">
        {#if co}<span><i class="dot" style="background:{famHue(co.slot)}"></i>{co.sector}</span>{#if co.country}<span>{co.country}</span>{/if}<span>inside your funds</span>
        {:else if held}<span>{isFund ? "Fund" : "Holding"} you hold</span>{#if held.currency !== cur}<span>trades in {held.currency}</span>{/if}
        {:else if parents.length}<span>Fund inside {parents.join(", ")}</span>
        {:else}<span>Not held</span>{/if}
      </div>
    </header>

    <section class="hero">
      <div class="label">You own</div>
      <Money value={yoursNow} currency={cur} size="xl" />
      <div class="sub">
        <PrivacyToggle />
        {#if invTotal > 0}<span class="chip"><span class="wt"><i style="width:{Math.min(100, (yoursNow / invTotal) * 100)}%"></i></span>{formatPct((yoursNow / invTotal) * 100, 1)} of your investments</span>{/if}
        {#if held}
          <span class="chip num">{formatUnits(held.units)} units{#if held.price != null}&nbsp;× {formatPlain(held.price)} {held.currency}{/if}</span>
          {#if held.gain != null}<span class="chip"><Money value={held.gain} currency={cur} signed tone="delta" /><span class="muted">vs book cost{#if held.gain_pct != null}&nbsp;({Number(held.gain_pct) > 0 ? "+" : ""}{held.gain_pct}%){/if}</span></span>{/if}
          {#if held.day_change != null}<span class="chip"><Money value={held.day_change} currency={cur} signed tone="delta" /><span class="muted">today</span></span>{/if}
        {:else if co}
          <span class="chip muted">through {[...new Set(co.via.map((v) => v.path[0]))].length} of your funds</span>
        {/if}
      </div>
    </section>

    <section class="card chart">
      <div class="ch">
        <div class="seg" role="radiogroup" aria-label="Chart">
          <button type="button" role="radio" aria-checked={mode === "price"} class:on={mode === "price"} disabled={!win} title={win ? "" : data?.price_note ?? ""} onclick={() => (show = "price")}>Price</button>
          <button type="button" role="radio" aria-checked={mode === "yours"} class:on={mode === "yours"} onclick={() => (show = "yours")}>What you own</button>
        </div>
        {#if mode === "price" && win}
          <div class="seg" role="radiogroup" aria-label="Range">
            {#each RANGES as r (r.id)}<button type="button" role="radio" aria-checked={range === r.id} class:on={range === r.id} onclick={() => (range = r.id)}>{r.label}</button>{/each}
          </div>
        {/if}
      </div>
      {#if error}
        <p class="error">Could not load this page's figures: {error}</p>
      {:else if !data}
        <p class="faint">Loading…</p>
      {:else if mode === "price" && win}
        <div class="stats">
          <span class="big num" class:up={win.change > 0} class:down={win.change < 0}>{win.pct == null ? "" : `${win.pct > 0 ? "+" : ""}${win.pct.toFixed(1)}%`}</span>
          <span class="faint">over {RANGES.find((r) => r.id === range)!.label} · {formatPlain(win.first)} → {formatPlain(win.last)} {data.currency ?? ""} · low {formatPlain(win.low)} · high {formatPlain(win.high)}</span>
        </div>
        <LineChart points={win.pts} label="{sym} price" format={(v) => `${formatPlain(v)} ${data?.currency ?? ""}`} tone={win.change > 0 ? "up" : win.change < 0 ? "down" : "neutral"} />
        {#if data.price_source === "fetched"}<p class="faint note">Looked up for this page; refreshed every 12 hours.</p>{/if}
      {:else}
        <p class="faint stats">{yoursPts.length > 1 ? `Your dollars in it at each check-in, through every fund.` : ""}{#if data.estimated_before}&nbsp;Before {formatDate(data.estimated_before)}, your first holdings file, its weights stand in.{/if}{#if !win && data.price_note}&nbsp;{data.price_note}{/if}</p>
        <LineChart points={yoursPts} label="Your dollars in {sym ?? co?.name}" format={(v) => formatMoney(v, cur)} step zero />
      {/if}
    </section>

    <div class="two">
      {#if isFund}
        <section class="card">
          <div class="cht"><h2 class="label">Inside {sym}</h2>{#if fund?.effective_holdings}<span class="faint small" title="1 ÷ the sum of squared weights inside the fund">acts like ~{fund.effective_holdings} equal names</span>{/if}</div>
          {#if mix.length}
            <div class="stack" title={mix.map((m) => `${m.family} ${formatPct(m.share * 100, 0)}`).join(" · ")}>{#each mix as m (m.family)}<i style="width:{m.share * 100}%;background:{familyHue(m.family) === '--f-none' ? 'var(--fill-soft)' : `var(${familyHue(m.family)})`}"></i>{/each}</div>
            <div class="keys">{#each mix.slice(0, 4) as m (m.family)}<span><i class="sq" style="background:{familyHue(m.family) === '--f-none' ? 'var(--fill-soft)' : `var(${familyHue(m.family)})`}"></i>{m.family} <b class="num">{formatPct(m.share * 100, 0)}</b></span>{/each}</div>
          {/if}
          {#if inside.length}
            {@const max = inside[0].share}
            <div class="rows">
              {#each inside as r (r.c.key)}
                <a class="row" href={hashFor({ name: "company", key: r.c.key })}>
                  <span class="n"><i class="dot" style="background:{famHue(r.c.slot)}"></i>{r.c.name}</span>
                  <span class="bar"><i style="width:{(r.share / max) * 100}%"></i></span>
                  <span class="num p">{formatPct(r.share)}</span>
                  <span class="v"><Money value={r.value} currency={cur} /></span>
                </a>
              {/each}
            </div>
          {:else}
            <p class="muted small">No holdings file for {sym} yet, so what it holds isn't known.</p>
          {/if}
          {#if fund?.as_of || snapInfo}<p class="faint note">Holdings file as of {formatDate((fund?.as_of ?? snapInfo?.as_of)!)}{#if snapInfo}&nbsp;· {snapInfo.constituents.toLocaleString("en-CA")} rows{/if}. Shares are of your dollars in {sym}.</p>{/if}
        </section>
      {:else if co}
        <section class="card">
          <h2 class="label">Through your funds</h2>
          <Lineage via={co.via} name={co.name} color={famHue(co.slot)} currency={cur} label={viaLabel} />
          <div class="links">
            {#each [...new Set(co.via.flatMap((v) => v.path))] as f (f)}<a href={hashFor({ name: "security", symbol: f })}>{f} ›</a>{/each}
          </div>
        </section>
      {/if}

      <section class="card side">
        {#if isFund && twins.length}
          <h2 class="label">Overlap with your other funds</h2>
          {#each twins as t (`${t.a}|${t.b}`)}
            {@const other = t.a === sym ? t.b : t.a}
            <a class="row twin" href={hashFor({ name: "security", symbol: other })}>
              <span class="n"><b>{other}</b></span>
              <span class="bar"><i class:hot={t.pct >= FLAG.pair} style="width:{t.pct}%"></i></span>
              <span class="num p" class:hot={t.pct >= FLAG.pair}>{t.pct.toFixed(0)}%</span>
            </a>
          {/each}
          <p class="faint note">Share of weight the two funds hold in common, through funds inside them.</p>
        {/if}
        {#if held && held.accounts.length}
          <h2 class="label" class:gap={isFund && twins.length}>Where you hold it</h2>
          {#each held.accounts as a (a.account)}
            <a class="row" href={hashFor({ name: "account", id: a.account })}><span class="n">{a.name}</span><span class="num faint small">{formatUnits(a.units)} units</span><span class="v">{#if a.value != null}<Money value={a.value} currency={cur} />{/if}</span></a>
          {/each}
        {/if}
        {#if co}
          <h2 class="label">At a glance</h2>
          <dl>
            <dt>Sector</dt><dd>{co.sector}</dd>
            {#if co.country}<dt>Country</dt><dd>{co.country}</dd>{/if}
            {#if co.currency}<dt>Trades in</dt><dd>{co.currency}</dd>{/if}
            <dt>Rank in your look-through</dt><dd>#{(x?.companies.findIndex((c) => c.key === co.key) ?? 0) + 1} of {x?.company_count.toLocaleString("en-CA")}</dd>
          </dl>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .sec { display: flex; flex-direction: column; gap: 18px; }
  .back { font-size: 12px; text-decoration: none; align-self: flex-start; }
  .back:hover { color: var(--accent); }
  .head { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 12px; }
  .head h1 { margin: 0; font-size: 26px; font-weight: 600; letter-spacing: -0.01em; }
  .tk, .nm { font-size: 14px; }
  .tags { flex-basis: 100%; display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 12px; color: var(--text-2); }
  .hero .sub { margin-top: 8px; display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; align-items: center; }
  .chip { display: inline-flex; gap: 6px; align-items: baseline; }
  .wt { display: inline-block; width: 48px; height: 6px; background: var(--surface-2); border-radius: 3px; overflow: hidden; align-self: center; }
  .wt i { display: block; height: 100%; background: var(--fill); }
  .ch { display: flex; justify-content: space-between; gap: 10px; flex-wrap: wrap; margin-bottom: 10px; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
  .seg button { padding: 4px 10px; font: inherit; font-size: 12px; color: var(--text-2); background: none; border: 0; cursor: pointer; }
  .seg button.on { background: var(--accent-soft); color: var(--text); }
  .seg button:disabled { opacity: 0.45; cursor: default; }
  .seg button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .stats { display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; font-size: 12px; margin: 0 0 6px; }
  .big { font-size: 18px; }
  .up { color: var(--positive); }
  .down { color: var(--negative); }
  .error { color: var(--negative); }
  .two { display: grid; grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr); gap: 20px; align-items: start; }
  .cht { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; flex-wrap: wrap; }
  h2.label { margin: 0 0 10px; font-weight: 500; }
  h2.gap { margin-top: 18px; }
  .small { font-size: 12px; }
  .stack { display: flex; gap: 1px; height: 10px; border-radius: 3px; overflow: hidden; background: var(--surface-2); }
  .stack i { display: block; height: 100%; }
  .keys { display: flex; flex-wrap: wrap; gap: 4px 14px; margin: 7px 0 10px; font-size: 12px; color: var(--text-2); }
  .keys b { color: var(--text); font-weight: 500; }
  .sq { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; }
  .dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; margin-right: 7px; vertical-align: 1px; }
  .rows { display: flex; flex-direction: column; }
  .row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(40px, 90px) 52px auto; gap: 10px; align-items: center; padding: 6px 0; border-top: 1px solid var(--border); font-size: 13px; color: var(--text); text-decoration: none; }
  .row:hover .n { color: var(--accent); }
  .row:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; border-radius: 4px; }
  .row .n { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row .bar { height: 6px; background: var(--surface-2); border-radius: 3px; overflow: hidden; }
  .row .bar i { display: block; height: 100%; background: var(--fill); }
  .row .bar i.hot { background: var(--warn); }
  .row .p { text-align: right; font-size: 12px; }
  .hot { color: var(--warn); }
  .row .v { text-align: right; min-width: 70px; }
  .side .row { grid-template-columns: minmax(0, 1fr) auto auto; }
  .side .row.twin { grid-template-columns: minmax(0, 1fr) minmax(40px, 100px) 44px; }
  .links { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
  .links a { font-size: 12px; padding: 3px 9px; border: 1px solid var(--border); border-radius: 999px; color: var(--text-2); text-decoration: none; }
  .links a:hover { color: var(--accent); border-color: var(--accent); }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; margin: 0; font-size: 13px; }
  dt { color: var(--text-3); }
  dd { margin: 0; }
  .note { font-size: 12px; margin: 10px 0 0; }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; }
  @media (max-width: 760px) {
    .two { grid-template-columns: 1fr; }
    .row { grid-template-columns: minmax(0, 1fr) 52px auto; }
    .row .bar { display: none; }
  }
</style>
