<script lang="ts">
  // Net worth over time (uPlot). Plain arrays in, one line or stacked groups out.
  import uPlot from "uplot";
  import { onMount } from "svelte";
  import { GROUPS, GROUP_LABEL, type Group, type GrowthPoint, type Point } from "../api";
  import { formatCompact, formatDate, formatMoney, formatSigned } from "../money";

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
  }: { series: Point[]; events?: { date: string; note: string }[]; currency?: string; blur?: boolean; groups?: boolean; growth?: GrowthPoint[] | null; range?: Range } = $props();

  let mode = $state<Mode>("line");
  let host: HTMLDivElement;
  let plot: uPlot | null = null;
  let hover = $state<{ i: number; left: number } | null>(null);

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

  // Added vs grown: a dashed line of where you would be on what you put in alone,
  // from the window's first point. The gap to net worth is what the market did.
  const addedBy = $derived(new Map((growth ?? []).map((p) => [p.date, Number(p.added)])));
  const addedLine = $derived.by(() => {
    if (!visible.length) return [];
    const a0 = addedBy.get(visible[0].date) ?? 0;
    const n0 = Number(visible[0].net_worth);
    let last = a0;
    return visible.map((p) => { last = addedBy.get(p.date) ?? last; return n0 + last - a0; });
  });

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
      cursor: { x: true, y: false, points: { show: false }, drag: { x: false, y: false } },
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
          hover = i == null ? null : { i, left: u.valToPos(u.data[0][i], "x") };
        }],
        draw: [(u) => { if (mode === "added") drawGap(u); drawEvents(u); }],
      },
    };
    if (mode === "line") {
      base.series.push({ stroke: accent, width: 2, fill: withAlpha(accent, 0.1), points: { show: false } });
    } else if (mode === "added") {
      base.series.push(
        { stroke: accent, width: 2, points: { show: false } },
        { stroke: css("--g-cash"), width: 1.5, dash: [5, 4], points: { show: false } },
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

  const hovered = $derived(hover ? visible[hover.i] : null);
  const hoverEvents = $derived(hovered ? events.filter((e) => e.date === hovered.date) : []);
  const tipLeft = $derived.by(() => {
    if (!hover || !host) return 0;
    const w = host.clientWidth;
    return hover.left > w / 2 ? hover.left - 180 : hover.left + 16;
  });
</script>

<div class="chart">
  <div class="head">
    <div class="seg" role="group" aria-label="Range">
      <button type="button" class:on={range === "3m"} disabled={!hasRange["3m"]} onclick={() => (range = "3m")}>3M</button>
      <button type="button" class:on={range === "1y"} disabled={!hasRange["1y"]} onclick={() => (range = "1y")}>1Y</button>
      <button type="button" class:on={range === "all"} onclick={() => (range = "all")}>All</button>
    </div>
    {#if groups}<div class="seg" role="group" aria-label="View">
      <button type="button" class:on={mode === "line"} onclick={() => (mode = "line")}>Line</button>
      <button type="button" class:on={mode === "groups"} onclick={() => (mode = "groups")}>By group</button>
      {#if growth?.length}<button type="button" class:on={mode === "added"} onclick={() => (mode = "added")} title="Net worth against what you put in; the gap is the market">Added vs grown</button>{/if}
    </div>{/if}
  </div>

  <div class="plot" bind:this={host} class:blur>
    {#if hovered}
      <div class="tip" style="left:{tipLeft}px">
        <div class="top"><b class="money num">{formatMoney(hovered.net_worth, currency)}</b><span>{formatDate(hovered.date, "long")}</span></div>
        {#if mode === "added" && hover}
          {@const added = addedLine[hover.i] - Number(visible[0].net_worth)}
          {@const grown = Number(hovered.net_worth) - addedLine[hover.i]}
          <div class="row"><span><i style="background:var(--g-cash)"></i>You added</span><span class="money num">{formatSigned(added, currency)}</span></div>
          <div class="row"><span><i style="background:{grown < 0 ? 'var(--negative)' : 'var(--accent)'}"></i>Market and other</span><span class="money num">{formatSigned(grown, currency)}</span></div>
          <div class="ev">since {formatDate(visible[0].date)}</div>
        {/if}
        {#each groups && mode !== "added" ? GROUPS : [] as k}
          <div class="row"><span><i style="background:var(--g-{k})"></i>{GROUP_LABEL[k]}</span><span class="money num">{formatMoney(k === "debt" ? -Number(hovered.by_group[k] ?? 0) : hovered.by_group[k] ?? 0, currency)}</span></div>
        {/each}
        {#each hoverEvents as e}<div class="ev">· {e.note}</div>{/each}
      </div>
    {/if}
    {#if visible.length === 0}
      <p class="empty">No history yet. Your first check-in adds the first point.</p>
    {/if}
  </div>

  {#if mode === "added"}
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
  .plot :global(.u-cursor-x) { border-right: 1px solid var(--text-2); }
  .plot :global(.uplot) { font-family: var(--font); }
  .tip { position: absolute; top: 6px; z-index: 2; background: var(--surface-2); border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px; font-size: 12px; pointer-events: none; min-width: 164px; }
  .tip .top { display: flex; flex-direction: column; margin-bottom: 4px; }
  .tip .top b { font-size: 13px; }
  .tip .top span { color: var(--text-3); }
  .tip .row { display: flex; justify-content: space-between; gap: 14px; color: var(--text-2); }
  .tip i { display: inline-block; width: 10px; height: 2px; vertical-align: middle; margin-right: 6px; border-radius: 1px; }
  .tip .ev { color: var(--text-3); margin-top: 4px; }
  .empty { color: var(--text-3); font-size: 13px; padding: 90px 0; text-align: center; margin: 0; }
  .legend { display: flex; gap: 14px; margin-top: 8px; font-size: 12px; color: var(--text-2); flex-wrap: wrap; }
  .legend i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; }
  .legend i.line { height: 2px; width: 10px; background: var(--text); vertical-align: 2px; }
  .legend i.dash { background: repeating-linear-gradient(90deg, var(--g-cash) 0 4px, transparent 4px 7px); width: 14px; }
  .num { font-variant-numeric: tabular-nums; }
</style>
