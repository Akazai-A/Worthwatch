<script lang="ts">
  // Net worth over time (uPlot). Plain arrays in, one line or stacked groups out.
  import uPlot from "uplot";
  import { onMount } from "svelte";
  import { GROUPS, GROUP_LABEL, type Group, type GrowthPoint, type Point } from "../api";
  import { addedLine as addedLineOf, windowTotals } from "../networth/growth";
  import { formatCompact, formatDate, formatMoney, formatSigned } from "../format/money";

  type Range = "3m" | "1y" | "all";
  type Mode = "line" | "groups" | "added";

  let {
    series,
    events = [],
    currency = "CAD",
    blur = false,
    groups = true,
    growth = null,
    range = $bindable("1y"),
  }: {
    series: Point[]; events?: { date: string; note: string }[]; currency?: string; blur?: boolean; groups?: boolean; growth?: GrowthPoint[] | null; range?: Range;
  } = $props();

  // Added vs grown is the default view once the growth series has loaded: it is the
  // net worth line plus where the change came from. Line and By group are one click away.
  let chosen = $state<Mode | null>(null);
  const mode = $derived<Mode>(chosen ?? (growth?.length ? "added" : "line"));
  let host: HTMLDivElement;
  let plot: uPlot | null = null;
  // Where the hovered day sits, in px inside .plot: x, the net worth y, the added-line y (or the window's
  // first value in line mode, so the caliper measures the gain since then), and the plotting box.
  type Hover = { i: number; left: number; y: number; y0: number; box: { l: number; t: number; w: number; h: number } };
  let hover = $state<Hover | null>(null);

  const toSec = (iso: string) => Date.UTC(+iso.slice(0, 4), +iso.slice(5, 7) - 1, +iso.slice(8, 10)) / 1000;

  const visible = $derived.by(() => {
    if (series.length === 0 || range === "all") return series;
    const last = toSec(series[series.length - 1].date);
    const days = range === "3m" ? 92 : 366;
    const cut = last - days * 86400;
    const inRange = series.filter((p) => toSec(p.date) >= cut);
    // keep one point before the cut so the line starts at the edge
    const firstIdx = series.length - inRange.length - 1;
    return firstIdx >= 0 ? [series[firstIdx], ...inRange] : inRange;
  });
  const hasRange = $derived.by(() => {
    if (series.length < 2) return { "3m": false, "1y": false };
    const span = toSec(series[series.length - 1].date) - toSec(series[0].date);
    return { "3m": span > 92 * 86400, "1y": span > 366 * 86400 };
  });

  const g = (p: Point, k: Group) => Number(p.by_group[k] ?? 0);

  // Added vs grown: a dashed line of where you would be on what you put in alone.
  const addedLine = $derived(addedLineOf(visible, growth ?? []));

  function data(): uPlot.AlignedData {
    const xs = visible.map((p) => toSec(p.date));
    if (mode === "line") return [xs, visible.map((p) => Number(p.net_worth))];
    if (mode === "added") return [xs, visible.map((p) => Number(p.net_worth)), addedLine];
    const cash = visible.map((p) => g(p, "cash"));
    const inv = visible.map((p, i) => cash[i] + g(p, "investments"));
    const prop = visible.map((p, i) => inv[i] + g(p, "property"));
    const debt = visible.map((p) => -g(p, "debt"));
    return [xs, prop, inv, cash, debt, visible.map((p) => Number(p.net_worth))];
  }

  function css(name: string): string {
    return getComputedStyle(host).getPropertyValue(name).trim();
  }

  function opts(width: number): uPlot.Options {
    const text = css("--text-3"), grid = css("--border"), accent = css("--accent");
    const base: uPlot.Options = {
      width,
      height: 240,
      // uPlot only tracks the pointer; the readout is drawn below (no dot, no floating box over the line).
      cursor: { x: false, y: false, points: { show: false }, drag: { x: false, y: false } },
      legend: { show: false },
      scales: { x: { time: true }, y: { range: (_u, min, max) => pad(min, max) } },
      axes: [
        {
          stroke: text, grid: { show: false }, ticks: { show: false }, font: "11px " + css("--figures"), gap: 8,
          values: (_u, vals) => vals.map((v) => fmtTick(v)),
        },
        {
          side: 1, stroke: text, grid: { stroke: grid, width: 1 }, ticks: { show: false }, font: "11px " + css("--figures"),
          size: 60, gap: 8, values: (_u, vals) => vals.map((v) => (blur ? "*****" : formatCompact(v))),
        },
      ],
      series: [{}],
      hooks: {
        setCursor: [(u) => {
          const i = u.cursor.idx;
          if (i == null) { hover = null; return; }
          const o = u.over, l = o.offsetLeft, t = o.offsetTop;
          const nwIdx = mode === "groups" ? 5 : 1;
          const ref = mode === "added" ? (u.data[2][i] as number) : (u.data[nwIdx][0] as number);
          hover = {
            i,
            left: l + u.valToPos(u.data[0][i], "x"),
            y: t + u.valToPos(u.data[nwIdx][i] as number, "y"),
            y0: t + u.valToPos(ref, "y"),
            box: { l, t, w: o.clientWidth, h: o.clientHeight },
          };
        }],
        draw: [(u) => { if (mode === "added") drawGap(u); drawEvents(u); }],
      },
    };
    if (mode === "line") {
      base.series.push({ stroke: accent, width: 2, fill: withAlpha(accent, 0.1), points: { show: false } });
    } else if (mode === "added") {
      base.series.push(
        { stroke: accent, width: 2, points: { show: false } },
        // What you added: a bone dashed line (cash blue means cash; brass is net worth).
        { stroke: css("--text-2"), width: 1.5, dash: [5, 4], points: { show: false } },
      );
    } else {
      const c = (k: Group) => css(`--g-${k}`);
      base.series.push(
        { stroke: "transparent", fill: withAlpha(c("property"), 0.55), points: { show: false } },
        { stroke: "transparent", fill: withAlpha(c("investments"), 0.55), points: { show: false } },
        { stroke: "transparent", fill: withAlpha(c("cash"), 0.55), points: { show: false } },
        { stroke: "transparent", fill: withAlpha(c("debt"), 0.55), points: { show: false } },
        { stroke: css("--text"), width: 2, points: { show: false } },
      );
      base.bands = [{ series: [1, 2] }, { series: [2, 3] }];
    }
    return base;
  }

  function pad(min: number, max: number): [number, number] {
    const span = max - min || Math.abs(max) || 1000;
    const lo = mode === "groups" ? Math.min(min, 0) : min;
    return [lo - span * 0.1, max + span * 0.1];
  }

  function fmtTick(sec: number): string {
    const d = new Date(sec * 1000);
    const m = d.toLocaleString("en-CA", { month: "short", timeZone: "UTC" });
    return d.getUTCMonth() === 0 ? String(d.getUTCFullYear()) : `${m} ${String(d.getUTCFullYear()).slice(2)}`;
  }

  function drawEvents(u: uPlot) {
    if (events.length === 0) return;
    const ctx = u.ctx;
    const [xMin, xMax] = [u.scales.x.min ?? 0, u.scales.x.max ?? 0];
    ctx.save();
    ctx.strokeStyle = css("--text-3");
    ctx.lineWidth = 1 * devicePixelRatio;
    const y0 = u.bbox.top + u.bbox.height;
    for (const e of events) {
      const s = toSec(e.date);
      if (s < xMin || s > xMax) continue;
      const x = u.valToPos(s, "x", true);
      ctx.beginPath();
      ctx.moveTo(x, y0);
      ctx.lineTo(x, y0 - 7 * devicePixelRatio);
      ctx.stroke();
    }
    ctx.restore();
  }

  /** Shade between net worth and the added line: brass where the market helped, red where it hurt. */
  function drawGap(u: uPlot) {
    const [xs, nw, ad] = u.data as [number[], number[], number[]];
    const ctx = u.ctx;
    const up = withAlpha(css("--accent"), 0.22), down = withAlpha(css("--negative"), 0.28);
    ctx.save();
    // One path per run of the same sign, so neighbouring slices leave no seams.
    const sign = (i: number) => nw[i] + nw[i + 1] >= ad[i] + ad[i + 1];
    let i = 0;
    while (i < xs.length - 1) {
      const up0 = sign(i);
      let j = i;
      while (j < xs.length - 1 && sign(j) === up0) j++;
      ctx.fillStyle = up0 ? up : down;
      ctx.beginPath();
      for (let k = i; k <= j; k++) ctx.lineTo(u.valToPos(xs[k], "x", true), u.valToPos(nw[k], "y", true));
      for (let k = j; k >= i; k--) ctx.lineTo(u.valToPos(xs[k], "x", true), u.valToPos(ad[k], "y", true));
      ctx.closePath();
      ctx.fill();
      i = j;
    }
    ctx.restore();
  }

  function withAlpha(hex: string, a: number): string {
    const m = /^#([0-9a-f]{6})$/i.exec(hex);
    if (!m) return hex;
    const n = parseInt(m[1], 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
  }

  function build() {
    plot?.destroy();
    plot = null;
    hover = null;
    if (!host || visible.length === 0) return;
    plot = new uPlot(opts(host.clientWidth), data(), host);
  }

  onMount(() => {
    build();
    const ro = new ResizeObserver(() => { if (plot && host) plot.setSize({ width: host.clientWidth, height: 240 }); });
    ro.observe(host);
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const onTheme = () => build();
    mq.addEventListener("change", onTheme);
    const mo = new MutationObserver(onTheme);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => { ro.disconnect(); mq.removeEventListener("change", onTheme); mo.disconnect(); plot?.destroy(); };
  });

  $effect(() => {
    // re-render when inputs change
    void visible; void mode; void blur; void events; void addedLine;
    if (host) build();
  });

  // The window's totals, shown under the chart: what you put in, and what the market and FX did.
  const totals = $derived(mode === "added" ? windowTotals(visible, addedLine) : null);

  const hovered = $derived(hover ? visible[hover.i] : null);
  const hoverEvents = $derived(hovered ? events.filter((e) => e.date === hovered.date) : []);
  // The gain the caliper measures: since the window opened (line, groups) or over what you added.
  const measured = $derived.by(() => {
    if (!hover || !hovered) return null;
    if (mode === "added") return Number(hovered.net_worth) - addedLine[hover.i];
    return Number(hovered.net_worth) - Number(visible[0].net_worth);
  });
  const clampX = (x: number, w: number) => (hover ? Math.max(hover.box.l, Math.min(hover.box.l + hover.box.w - w, x - w / 2)) : 0);
</script>

<div class="chart">
  <div class="head">
    <div class="seg" role="group" aria-label="Range">
      <button type="button" class:on={range === "3m"} disabled={!hasRange["3m"]} onclick={() => (range = "3m")}>3M</button>
      <button type="button" class:on={range === "1y"} disabled={!hasRange["1y"]} onclick={() => (range = "1y")}>1Y</button>
      <button type="button" class:on={range === "all"} onclick={() => (range = "all")}>All</button>
    </div>
    {#if groups}<div class="seg" role="group" aria-label="View">
      <button type="button" class:on={mode === "line"} onclick={() => (chosen = "line")}>Line</button>
      <button type="button" class:on={mode === "groups"} onclick={() => (chosen = "groups")}>By group</button>
      {#if growth?.length}<button type="button" class:on={mode === "added"} onclick={() => (chosen = "added")} title="Net worth against what you put in; the gap is the market">Added vs grown</button>{/if}
    </div>{/if}
  </div>

  <div class="plot" bind:this={host} class:blur>
    {#if hovered && hover}
      {@const b = hover.box}
      <!-- Caliper: a bracket from the reference line to net worth, labelled with what it measures. -->
      {@const top = Math.min(hover.y, hover.y0)}
      {@const h = Math.max(1, Math.abs(hover.y0 - hover.y))}
      <div class="hair" style="left:{hover.left}px;top:{b.t}px;height:{b.h}px"></div>
      <div class="cal" class:neg={(measured ?? 0) < 0} style="left:{hover.left}px;top:{top}px;height:{h}px"></div>
      <div class="cal-tag money num" class:neg={(measured ?? 0) < 0} class:flip={hover.left > b.l + b.w * 0.75} style="left:{hover.left}px;top:{top + h / 2}px">
        {formatSigned(measured ?? 0, currency)}<span>{mode === "added" ? "market" : `since ${formatDate(visible[0].date)}`}</span>
      </div>
      <div class="tag x" style="left:{clampX(hover.left, 92)}px;top:{b.t + b.h + 4}px">{formatDate(hovered.date, "long")}</div>
      <div class="tag y money num" style="left:{b.l + b.w + 4}px;top:{hover.y}px">{formatCompact(Number(hovered.net_worth))}</div>
    {/if}
    {#if visible.length === 0}
      <p class="empty">No history yet. Your first check-in adds the first point.</p>
    {/if}
  </div>

  {#if hovered && hover}
    <p class="totals readout">
      <b class="money num">{formatMoney(hovered.net_worth, currency)}</b> on {formatDate(hovered.date, "long")}
      {#if mode === "added"}
        {@const added = addedLine[hover.i] - Number(visible[0].net_worth)}
        {@const grown = Number(hovered.net_worth) - addedLine[hover.i]}
        <span class="sep">·</span> you added <b class="money num">{formatSigned(added, currency)}</b>, the market <b class="money num" class:neg={grown < 0}>{formatSigned(grown, currency)}</b> <span class="faint">since {formatDate(visible[0].date)}</span>
      {:else if groups}
        {#each GROUPS as k}<span class="sep">·</span><span class="gk"><i style="background:var(--g-{k})"></i>{GROUP_LABEL[k]} <b class="money num">{formatCompact(k === "debt" ? -Number(hovered.by_group[k] ?? 0) : Number(hovered.by_group[k] ?? 0))}</b></span>{/each}
      {/if}
      {#each hoverEvents as e}<span class="sep">·</span><span class="faint">{e.note}</span>{/each}
    </p>
  {:else if mode === "line"}
    <p class="totals hint faint"><span class="on-hover">Hover</span><span class="on-touch">Tap</span> the line to read any day.</p>
  {:else if mode === "added"}
    {#if totals}
      <p class="totals">
        Since {formatDate(totals.from, "long")}: you added <b class="money num">{formatSigned(totals.added, currency)}</b>,
        the market and other changes <b class="money num" class:neg={totals.grown < 0}>{formatSigned(totals.grown, currency)}</b>{#if totals.fromYou != null}<span class="faint">&nbsp;· {totals.fromYou}% came from you</span>{/if}
      </p>
    {/if}
    <div class="legend">
      <span><i class="line" style="background:var(--accent)"></i>Net worth</span>
      <span><i class="line dash"></i>What you added (saving, contributions, debt paid down)</span>
      <span><i style="background:var(--accent);opacity:.45"></i>Market helped</span>
      <span><i style="background:var(--negative);opacity:.5"></i>Market hurt</span>
    </div>
  {:else if mode === "groups"}
    <div class="legend">
      {#each GROUPS as k}<span><i style="background:var(--g-{k})"></i>{GROUP_LABEL[k]}</span>{/each}
      <span><i class="line"></i>Net worth</span>
    </div>
  {/if}
</div>

<style>
  .head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; gap: 8px; flex-wrap: wrap; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 7px; overflow: hidden; }
  .seg button { background: none; border: 0; color: var(--text-2); padding: 4px 10px; font: inherit; font-size: 12px; cursor: pointer; }
  .seg button.on { background: var(--surface-2); color: var(--text); }
  .seg button:disabled { color: var(--text-3); cursor: default; }
  .seg button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .plot { position: relative; min-height: 240px; }
  /* Hover: a caliper measures the day's gain; tags sit on the axes, details go in the line under the chart.
     Nothing sits on top of the line it describes. */
  .hair, .tag, .cal, .cal-tag { position: absolute; pointer-events: none; z-index: 2; }
  .hair { width: 0; border-left: 1px dashed var(--text-3); }
  .tag { font-size: 11px; line-height: 18px; padding: 0 6px; border-radius: 3px; white-space: nowrap; background: var(--text); color: var(--bg); font-family: var(--figures); box-shadow: 0 0 0 6px var(--bg); }
  .tag.y { transform: translateY(-50%); background: var(--accent); box-shadow: 0 0 0 2px var(--bg); }
  .cal { width: 9px; margin-left: -4px; border: 1.5px solid var(--accent); border-left: 0; border-right: 0; }
  .cal::before { content: ""; position: absolute; left: 4px; top: 0; bottom: 0; border-left: 1.5px solid var(--accent); }
  .cal.neg, .cal.neg::before { border-color: var(--negative); }
  .cal-tag { transform: translate(10px, -50%); font-size: 12px; font-weight: 600; color: var(--accent); background: var(--bg); padding: 1px 5px; border-radius: 3px; white-space: nowrap; display: flex; flex-direction: column; line-height: 1.2; }
  .cal-tag span { font-family: var(--font); font-weight: 400; font-size: 11px; color: var(--text-3); }
  .cal-tag.flip { transform: translate(calc(-100% - 10px), -50%); text-align: right; }
  .cal-tag.neg { color: var(--negative); }
  .readout .sep { margin: 0 6px; color: var(--text-3); }
  .readout .gk i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 4px; }
  .plot :global(.uplot) { font-family: var(--font); }
  /* A new range or view draws in from the left, so the eye follows time forward. */
  @media (prefers-reduced-motion: no-preference) {
    .plot :global(.uplot canvas) { animation: draw 560ms var(--ease-out) both; }
  }
  @keyframes draw { from { clip-path: inset(0 100% 0 0); } to { clip-path: inset(0 0 0 0); } }
  .empty { color: var(--text-3); font-size: 13px; padding: 90px 0; text-align: center; margin: 0; }
  .legend { display: flex; gap: 14px; margin-top: 8px; font-size: 12px; color: var(--text-2); flex-wrap: wrap; }
  .legend i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; }
  .legend i.line { height: 2px; width: 10px; background: var(--text); vertical-align: 2px; }
  .legend i.dash { background: repeating-linear-gradient(90deg, var(--text-2) 0 4px, transparent 4px 7px); width: 14px; }
  .totals { margin: 10px 0 0; font-size: 13px; color: var(--text-2); }
  .totals b { color: var(--text); font-weight: 600; }
  .totals b.neg { color: var(--negative); }
  .num { font-variant-numeric: tabular-nums; }
</style>
