<script lang="ts">
  import PrivacyToggle from "../../lib/components/shell/PrivacyToggle.svelte";
  // Changes: what moved in your investments between two dates, told through your funds.
  // A waterfall of funds, then what moved underneath (companies and sectors, in dollars),
  // then what each fund bought and sold itself. Accounts and net worth live on Home.
  // All figures come from GET /api/changes; this file lays them out.
  import type { BucketDelta, CompanyDelta, HoldingDiff, LedgerView } from "../../lib/api";
  import { formatDate, formatPlain, formatSigned, formatUnits, formatPct } from "../../lib/format/money";
  import { PRESETS, changeLine, fundSteps, minorLine, presetFrom, presetTo, rangeLabel, type BridgeStep, type Preset } from "../../lib/portfolio/changes";
  import { familyHue, familyOf } from "../../lib/portfolio/lookthrough";
  import { changes as store } from "../../lib/stores/portfolio/changes.svelte";
  import { lookthrough } from "../../lib/stores/portfolio/lookthrough.svelte";
  import { prefs } from "../../lib/stores/prefs.svelte";
  import { hashFor } from "../../lib/stores/route.svelte";
  import Money from "../../lib/components/amounts/Money.svelte";
  import MarketLine from "../../lib/components/portfolio/MarketLine.svelte";
  import Bridge from "../../lib/charts/Bridge.svelte";
  import LineChart from "../../lib/charts/LineChart.svelte";
  import MultiLine from "../../lib/charts/MultiLine.svelte";
  import SplitStrip from "../../lib/components/amounts/SplitStrip.svelte";
  import DeltaBar from "../../lib/components/amounts/DeltaBar.svelte";
  import WeightMove from "../../lib/components/amounts/WeightMove.svelte";

  let { view }: { view: LedgerView } = $props();
  const cur = $derived(view.base_currency);
  const today = $derived(view.as_of);
  // The period: a preset or two typed dates; the server fills in "since last check-in".
  const from = $derived(presetFrom(store.preset, today, store.current?.first_date ?? null, store.custom));
  const to = $derived(presetTo(store.preset, today, store.custom));
  $effect(() => {
    void store.ensure(view.version, from, to);
    void lookthrough.ensure(view.version);
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
  // Draft-only switch for the sign-off board: localStorage ww.variant = "tb" or "tc".
  const variant = (() => { try { return (localStorage.getItem("ww.variant") ?? "").split(","); } catch { return []; } })();

  // Column and bridge labels carry the year only when the two ends differ in it.
  const endLabel = (d: string) => formatDate(d, c && c.from.slice(0, 4) !== c.to.slice(0, 4) ? "long" : "short");
  const hs = $derived(c?.holdings ?? []);
  const inv = $derived({
    from: hs.reduce((s, h) => s + Number(h.from_value ?? 0), 0),
    to: hs.reduce((s, h) => s + Number(h.to_value ?? 0), 0),
    change: hs.reduce((s, h) => s + Number(h.change ?? 0), 0),
  });
  const steps = $derived(fundSteps(hs));
  // One tile (and one line) per fund, biggest dollar move first; the line is its price over the period.
  const LINE_HUES = ["--f-tech", "--f-fin", "--f-cons", "--f-ind", "--text-2"];
  const tiles = $derived([...hs].sort((a, b) => Math.abs(Number(b.change ?? 0)) - Math.abs(Number(a.change ?? 0))).map((h, i) => ({
    h,
    pm: priceMove(h),
    pts: (v?.paths[h.symbol] ?? []).map((p) => ({ date: p.date, v: Number(p.close) })),
    color: `var(${LINE_HUES[i % LINE_HUES.length]})`,
  })));
  const lines = $derived(tiles.filter((t) => t.pts.length > 1).map((t) => ({ label: t.h.symbol, color: t.color, pts: t.pts.map((p) => ({ date: p.date, v: (p.v / t.pts[0].v - 1) * 100 })) })));
  const bySym = $derived(new Map(hs.map((h) => [h.symbol, h])));
  const fundName = (sym: string) => lookthrough.current?.lookthrough.funds.find((f) => f.symbol === sym)?.name ?? null;
  const holdParts = (h: HoldingDiff) => [
    { label: "market", value: Number(h.market ?? 0) },
    { label: "you bought or sold", value: Number(h.trades ?? 0) },
    { label: "fx", value: Number(h.fx ?? 0) },
  ];
  const priceMove = (h: HoldingDiff) => (h.from_price != null && h.to_price != null && Number(h.from_price) > 0 ? (Number(h.to_price) / Number(h.from_price) - 1) * 100 : null);
  const unitMove = (h: HoldingDiff) => Number(h.to_units) - Number(h.from_units);
  const maxHolding = $derived(Math.max(0, ...hs.map((h) => Math.abs(Number(h.change ?? 0)))));

  // Underneath: look-through needs a file at both ends.
  const anyFiles = $derived((v?.funds_resolved_to ?? 0) > 0 && (v?.funds_resolved_from ?? 0) > 0);
  const noOldFiles = $derived((v?.funds_resolved_to ?? 0) > 0 && (v?.funds_resolved_from ?? 0) === 0);
  const cos = $derived<CompanyDelta[]>(anyFiles ? (v?.drift.by_company ?? []) : []);
  // Gainers first, then the drops; all 40 in a list that scrolls inside its card.
  const shownCos = $derived([...cos].sort((a, b) => Number(b.delta) - Number(a.delta)));
  const maxCo = $derived(Math.max(0.01, ...cos.map((x) => Math.abs(Number(x.delta)))));
  const sectorMove = (b: BucketDelta) => Number(b.to_value) - Number(b.from_value);
  const sectors = $derived<BucketDelta[]>(anyFiles ? (v?.drift.by_sector ?? []).filter((b) => Math.abs(sectorMove(b)) >= 0.5).sort((a, b) => sectorMove(b) - sectorMove(a)) : []);
  const maxSec = $derived(Math.max(0.01, ...sectors.map((b) => Math.abs(sectorMove(b)))));
  const hue = (slot?: string) => {
    const h = slot ? familyHue(familyOf(slot)) : "--f-none";
    return h === "--f-none" ? "var(--fill)" : `var(${h})`;
  };
  const THRESHOLDS = ["0.1", "0.5", "1.0"];
</script>

<div class="changes">
  <div class="top faint"><span>as of {formatDate(view.as_of, "long")} · {cur}</span>{#if view.accounts.length}<MarketLine {view} />{/if}</div>

  {#if view.accounts.length === 0}
    <section class="card empty">
      <h2>Nothing to compare yet</h2>
      <p class="muted">Changes shows what moved in your funds between two check-ins. Add accounts and enter balances on <a href="#/check-in">Check-in</a>; the second check-in gives this screen something to say.</p>
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
      <span class="eye"><PrivacyToggle /></span>
    </div>

    {#if store.error && !v}
      <p class="error">Could not load changes: {store.error}</p>
    {:else if !v || !c}
      <p class="faint">Loading…</p>
    {:else if c.days === 0}
      <section class="card empty">
        <h2>One check-in so far</h2>
        <p class="muted">Nothing was recorded before {formatDate(c.to, "long")}. After the next check-in this screen shows what moved in your funds.</p>
      </section>
    {:else if hs.length === 0}
      <section class="card empty">
        <h2>No funds to follow</h2>
        <p class="muted">Changes follows your funds and what is inside them. Add holdings to an investment account on <a href="#/check-in">Check-in</a>. Net worth over the same days is on <a href="#/">Home</a>.</p>
      </section>
    {:else}
      {#if variant.includes("tc")}
        <section class="card panel">
          <div class="head"><h2 class="label">Your funds: what moved them</h2><span class="faint small">one step per fund · <span class="on-hover">hover</span><span class="on-touch">tap</span> a step for price, units and FX</span></div>
          <div class="causes faint small"><span><i class="k-market"></i>price and FX</span><span><i class="k-money"></i>money you put in or took out</span></div>
          <Bridge title="Investments" start={inv.from} end={inv.to} {steps} startLabel={endLabel(c.from)} endLabel={endLabel(c.to)} currency={cur} blur={prefs.blur} tip={stepTip} />
        </section>
      {:else if variant.includes("tb")}
        <section class="card panel">
          <div class="head"><h2 class="label">Your funds, side by side</h2><span class="faint small">price move since {endLabel(c.from)}, in each fund's own currency</span></div>
          <MultiLine series={lines} label="Price move by fund" format={(v) => `${v > 0 ? "+" : ""}${v.toFixed(1)}%`} />
          <div class="mini">
            {#each tiles as t (t.h.symbol)}
              <a class="mrow" href={hashFor({ name: "security", symbol: t.h.symbol })}>
                <span class="name"><i class="dot" style="background:{t.color}"></i><b>{t.h.symbol}</b> <span class="faint">{fundName(t.h.symbol) ?? ""}</span></span>
                <span class="div"><i class:pos={Number(t.h.change ?? 0) > 0} class:neg={Number(t.h.change ?? 0) < 0} style="width:{(Math.abs(Number(t.h.change ?? 0)) / Math.max(0.01, maxHolding)) * 50}%"></i></span>
                <span class="num amt"><Money value={t.h.change ?? 0} currency={cur} signed tone="delta" /></span>
              </a>
            {/each}
          </div>
        </section>
      {:else}
        <section class="card panel">
          <div class="head"><h2 class="label">Your funds</h2><span class="faint small">price over the period, in each fund's own currency · your dollars gained or lost</span></div>
          <div class="tiles">
            {#each tiles as t (t.h.symbol)}
              {@const ch = Number(t.h.change ?? 0)}
              <a class="tile" href={hashFor({ name: "security", symbol: t.h.symbol })}>
                <span class="tt"><b>{t.h.symbol}</b>{#if t.pm != null}<span class="num" class:up={t.pm > 0} class:down={t.pm < 0}>{t.pm > 0 ? "+" : ""}{t.pm.toFixed(1)}%</span>{/if}</span>
                <span class="tn">{fundName(t.h.symbol) ?? t.h.currency}</span>
                {#if t.pts.length > 1}<LineChart bare height={48} points={t.pts} label="{t.h.symbol} price" format={(v) => `${formatPlain(v)} ${t.h.currency}`} tone={t.pm != null && t.pm > 0 ? "up" : t.pm != null && t.pm < 0 ? "down" : "neutral"} />{:else}<span class="nospark faint">no price line</span>{/if}
                <span class="tb">{#if t.h.change == null}<span class="warn">unpriced</span>{:else}<Money value={ch} currency={cur} signed tone="delta" />{/if}{#if holdParts(t.h).filter((p) => Math.abs(p.value) >= 0.005).length > 1}<SplitStrip parts={holdParts(t.h)} currency={cur} width="64px" height={5} />{/if}</span>
                {#if unitMove(t.h) !== 0}<span class="tu faint">{unitMove(t.h) > 0 ? "bought" : "sold"} {formatUnits(String(Math.abs(unitMove(t.h))))} units</span>{/if}
              </a>
            {/each}
          </div>
        </section>
      {/if}

        <div class="pair">
          <section class="card panel">
            <div class="head"><h2 class="label">Underneath: companies</h2><span class="faint small">your dollars in each, through every fund</span></div>
            {#if cos.length === 0}
              <p class="muted empty-line">{noOldFiles ? `No holdings file is old enough for ${formatDate(v.from)}, so there is nothing to compare underneath.` : "No holdings files yet, so nothing to look under."}</p>
            {:else}
              <div class="list scroll">
                {#each shownCos as x (x.key)}
                  {@const d = Number(x.delta)}
                  <a class="mrow" href={hashFor({ name: "company", key: x.key })} title="{x.name}: {formatSigned(d, cur)} ({formatPlain(x.from_value, 0)} → {formatPlain(x.to_value, 0)}) · {x.sector}">
                    <span class="name"><i class="dot" style="background:{hue(x.slot)}"></i>{x.ticker && x.ticker !== "-" ? x.ticker : x.name}{#if Number(x.from_value) === 0}<span class="tag up">new</span>{:else if Number(x.to_value) === 0}<span class="tag down">gone</span>{/if}</span>
                    <span class="div"><i class:pos={d > 0} class:neg={d < 0} style="width:{(Math.abs(d) / maxCo) * 50}%"></i></span>
                    <span class="num amt"><Money value={d} currency={cur} signed tone="delta" /></span>
                  </a>
                {/each}
              </div>
            {/if}
          </section>
          <section class="card panel">
            <div class="head"><h2 class="label">Underneath: sectors</h2><span class="faint small">dollars · share of you on hover</span></div>
            {#if sectors.length === 0}
              <p class="muted empty-line">{anyFiles ? "No sector moved a dollar." : "Needs a holdings file at both ends of the period."}</p>
            {:else}
              <div class="list">
                {#each sectors as b (b.label)}
                  {@const d = sectorMove(b)}
                  <div class="mrow" title="{b.label}: {formatSigned(d, cur)} · share {formatPct(b.from_pct)} → {formatPct(b.to_pct)} ({Number(b.delta) > 0 ? '+' : ''}{b.delta} pt)">
                    <span class="name"><i class="dot" style="background:{hue(b.slot)}"></i>{b.label}</span>
                    <span class="div"><i class:pos={d > 0} class:neg={d < 0} style="width:{(Math.abs(d) / maxSec) * 50}%"></i></span>
                    <span class="num amt"><Money value={d} currency={cur} signed tone="delta" /></span>
                  </div>
                {/each}
              </div>
            {/if}
          </section>
        </div>

      <section class="card panel">
        <div class="head">
          <h2 class="label">Inside your funds <span class="faint sub-h">· what each fund bought and sold itself</span></h2>
          <span class="faint small thr">moves of
            <span class="seg mini" role="radiogroup" aria-label="Threshold">
              {#each THRESHOLDS as t}<button type="button" role="radio" aria-checked={store.threshold === t} class:on={store.threshold === t} onclick={() => store.setThreshold(t)}>{t}</button>{/each}
            </span> pt or more</span>
        </div>
        {#if v.funds.length === 0}
          <p class="muted empty-line">
            {#if v.funds_resolved_to === 0}No holdings files yet, so nothing to compare. Files arrive on <a href="#/portfolio">Portfolio</a>.
            {:else if v.same_file.length && !v.no_earlier_file.length}No fund published a new holdings file in this period{#if v.drift.to_as_of}&nbsp;(latest {formatDate(v.drift.to_as_of)}){/if}. Pick a longer period to compare files.
            {:else}Only one holdings file per fund so far; the first comparison appears once a newer file is fetched.{/if}
          </p>
        {:else}
          <div class="feed">
            {#each v.funds.filter((f) => f.changes.length > 0) as f (f.symbol)}
              <div class="item">
                <span class="who"><a class="sym" href={hashFor({ name: "security", symbol: f.symbol })}>{f.symbol}</a><span class="when">{formatDate(f.from)} → {formatDate(f.to)}{#if !f.held}&nbsp;· inside a fund you hold{/if}</span></span>
                <span class="what">
                  {#if f.changes.length > 0}
                  {@const maxW = Math.max(...f.changes.slice(0, 3).flatMap((ch) => [Number(ch.from_weight), Number(ch.to_weight)]), 0.1)}
                  {#each f.changes.slice(0, 3) as ch (ch.key)}
                    {@const d = Number(ch.delta)}
                    <span class="line" title={changeLine(ch)}>
                      <span class="tag" class:up={ch.kind === "added"} class:down={ch.kind === "removed"}>{ch.kind === "reweighted" ? (d > 0 ? "up" : "down") : ch.kind}</span>
                      <span class="cname">{ch.ticker && ch.ticker !== "-" ? ch.ticker : ch.name}</span>
                      <WeightMove from={Number(ch.from_weight)} to={Number(ch.to_weight)} max={maxW} />
                      <span class="num pts" class:up={d > 0} class:down={d < 0}>{d > 0 ? "+" : "−"}{Math.abs(d).toFixed(2)}</span>
                      {#if ch.your_value != null && Number(ch.your_value) !== 0}<span class="eff"><Money value={ch.your_value} currency={cur} signed tone="delta" /></span>{/if}
                    </span>
                  {/each}
                  {/if}
                  {#if f.changes.length > 3}<span class="sub2">+{f.changes.length - 3 + f.hidden} more</span>{/if}
                  <span class="sub2" title={minorLine(f)}>{f.to_constituents.toLocaleString("en-CA")} names · {formatPct(f.turnover_pct)} turnover{#if f.your_value != null}&nbsp;· <Money value={f.your_value} currency={cur} size="s" /> of yours{/if}</span>
                </span>
              </div>
            {/each}
          </div>
          {#if v.funds.some((f) => f.changes.length === 0) || v.same_file.length}
            {@const still = v.funds.filter((f) => f.changes.length === 0).map((f) => f.symbol)}
            <p class="faint note">{#if still.length}Nothing over {Number(store.threshold).toFixed(1)} pt in {still.join(", ")}.&nbsp;{/if}{#if v.same_file.length}No new file this period for {v.same_file.join(", ")}. {/if}&nbsp;Mark = old weight, bar = move.</p>
          {/if}
        {/if}
      </section>
    {/if}
  {/if}
</div>

{#snippet stepTip(step: BridgeStep)}
  {@const h = bySym.get(step.label)}
  {#if h}
    {@const pm = priceMove(h)}
    <div class="st">
      <div class="t"><a class="sym" href={hashFor({ name: "security", symbol: h.symbol })}>{h.symbol} ›</a><Money value={h.change ?? 0} currency={cur} signed tone="delta" /></div>
      {#if fundName(h.symbol)}<div class="ft">{fundName(h.symbol)}</div>{/if}
      <div class="ft num">{#if h.from_value != null}<Money value={h.from_value} currency={cur} size="s" />{:else}new{/if} → {#if h.to_value != null}<Money value={h.to_value} currency={cur} size="s" />{:else}—{/if}</div>
      <div class="ft num">price {#if h.from_price != null}{formatPlain(h.from_price)} → {/if}{h.to_price != null ? formatPlain(h.to_price) : "none"} {h.currency}{#if pm != null}&nbsp;({pm > 0 ? "+" : ""}{pm.toFixed(1)}%){/if}{#if unitMove(h) !== 0}&nbsp;· {unitMove(h) > 0 ? "bought" : "sold"} {formatUnits(String(Math.abs(unitMove(h))))} units{/if}</div>
      {#if holdParts(h).filter((p) => Math.abs(p.value) >= 0.005).length > 1}<SplitStrip parts={holdParts(h)} currency={cur} height={6} legend blur={prefs.blur} />{/if}
    </div>
  {/if}
{/snippet}

<style>
  .changes { display: flex; flex-direction: column; gap: 20px; }
  .top { display: flex; justify-content: space-between; align-items: center; gap: 12px; font-size: 12px; flex-wrap: wrap; }
  .error { color: var(--negative); }
  .empty { padding: 28px 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 20px; font-weight: 600; }
  .empty p { margin: 0; max-width: 56ch; }
  .eye { margin-left: auto; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 210px), 1fr)); gap: 12px; }
  .tile { display: flex; flex-direction: column; gap: 4px; padding: 12px; border: 1px solid var(--border); border-radius: 8px; color: var(--text); text-decoration: none; min-width: 0; }
  .tile:hover { border-color: var(--accent); }
  .tile:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tt { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; font-size: 15px; }
  .tt b { font-weight: 600; }
  .tn { font-size: 11px; color: var(--text-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-bottom: 6px; }
  .tb { display: flex; justify-content: space-between; align-items: center; gap: 8px; margin-top: 6px; font-size: 14px; }
  .tu { font-size: 11px; }
  .nospark { font-size: 11px; height: 48px; display: flex; align-items: center; }
  .mini { margin-top: 10px; }
  .range { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; font-size: 12px; }
  .seg { display: inline-flex; flex-wrap: wrap; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
  .seg button { padding: 5px 11px; font: inherit; font-size: 12px; color: var(--text-2); background: none; border: 0; cursor: pointer; }
  .seg button.on { background: var(--accent-soft); color: var(--text); }
  .seg button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .seg.mini { vertical-align: middle; margin: 0 2px; flex-wrap: nowrap; }
  .seg.mini button { padding: 1px 7px; font-size: 11px; font-family: var(--figures); }
  .custom { display: inline-flex; align-items: center; gap: 6px; }
  .custom input { font: inherit; font-size: 12px; color: var(--text); background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 3px 6px; color-scheme: inherit; }
    .panel { display: flex; flex-direction: column; min-width: 0; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 4px 12px; flex-wrap: wrap; margin-bottom: 8px; }
  h2 { margin: 0; font-weight: 500; }
  .small { font-size: 12px; }
  .sub-h { text-transform: none; letter-spacing: 0; }
  .causes { display: flex; gap: 14px; flex-wrap: wrap; margin: -2px 0 8px; }
  .causes i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; }
  .k-market { background: var(--positive); }
  .k-money { background: var(--fill); }
  .st { display: flex; flex-direction: column; gap: 6px; }
  .st .t { display: flex; justify-content: space-between; gap: 10px; font-weight: 600; }
  .st .ft { color: var(--text-2); }
  /* Companies beside sectors when both get room for their bars. */
  .pair { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr)); gap: 20px; align-items: start; }
  .list.scroll { max-height: 300px; overflow-y: auto; overscroll-behavior: contain; padding-right: 4px; }
  .mrow { display: grid; grid-template-columns: minmax(0, 150px) minmax(40px, 1fr) auto; gap: 10px; align-items: center; font-size: 12px; padding: 4px 0; }
  .mrow .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .amt { text-align: right; min-width: 64px; }
  .dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; margin-right: 7px; vertical-align: 1px; }
  .div { position: relative; height: 10px; background: var(--surface-2); border-radius: 3px; overflow: hidden; }
  .div::before { content: ""; position: absolute; left: 50%; top: 0; bottom: 0; width: 1px; background: var(--border); }
  .div i { position: absolute; top: 0; bottom: 0; min-width: 1px; }
  .div i.pos { left: 50%; background: var(--positive); border-radius: 0 3px 3px 0; }
  .div i.neg { right: 50%; background: var(--negative); border-radius: 3px 0 0 3px; }
  .pts { text-align: right; min-width: 44px; }
  .up { color: var(--positive); }
  .down { color: var(--negative); }
  .sym { font-weight: 500; color: var(--text); text-decoration: none; }
  .sym:hover { color: var(--accent); }
  a.mrow { color: var(--text); text-decoration: none; }
  a.mrow:hover .name { color: var(--accent); }
  .sub2 { display: block; font-size: 11px; color: var(--text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .warn { color: var(--warn); }
  .note { font-size: 12px; margin: 10px 0 0; }
  .tag { font-size: 11px; padding: 0 5px; border-radius: 4px; background: var(--surface-2); color: var(--text-2); margin-left: 6px; }
  .tag.up { color: var(--positive); }
  .tag.down { color: var(--negative); }
  .thr { display: inline-flex; align-items: center; gap: 4px; flex-wrap: wrap; }
  .empty-line { margin: 4px 0 0; font-size: 13px; }
  .feed { display: flex; flex-direction: column; max-height: 420px; overflow-y: auto; overscroll-behavior: contain; }
  .item { display: grid; grid-template-columns: 128px minmax(0, 1fr); gap: 14px; padding: 9px 0; border-top: 1px solid var(--border); font-size: 13px; }
  .item:first-of-type { border-top: 0; }
  .who { display: flex; flex-direction: column; }
  .when { font-size: 11px; color: var(--text-3); }
  .what { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .line { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .line .tag { margin-left: 0; width: 58px; text-align: center; font-size: 12px; }
  .cname { min-width: 56px; }
  .eff { font-size: 12px; color: var(--text-2); margin-left: auto; white-space: nowrap; }
  @media (max-width: 760px) {
    .item { grid-template-columns: 1fr; gap: 4px; }
  }
</style>
