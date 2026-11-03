<script lang="ts">
  // Net worth so far (solid) running into the projection (dashed) with its low/high
  // band, the money you put in as a shaded floor, the target as a dotted rule, and
  // markers for milestones and debt payoffs. Hover shows the month's figures.
  import type { DebtPayoff, Milestone, Point, ProjPoint } from "../api";
  import { fan, niceStep, monthYear } from "../networth/plan";
  import { formatCompact, formatMoney } from "../format/money";

  let {
    history,
    points,
    start,
    startNetWorth,
    milestones = [],
    debts = [],
    target = null,
    currency = "CAD",
    blur = false,
    height = 300,
    compact = false,
  }: {
    history: Point[]; points: ProjPoint[]; start: string; startNetWorth: string;
    milestones?: Milestone[]; debts?: DebtPayoff[]; target?: number | null;
    currency?: string; blur?: boolean; height?: number; compact?: boolean;
  } = $props();

  let width = $state(700);
  let hoverX = $state<number | null>(null);
  const L = 4, R = 58, T = 14, B = 24;

  const f = $derived(fan(history, points, start, startNetWorth, target, !compact));
  const x = (t: number) => L + ((t - f.t0) / (f.t1 - f.t0 || 1)) * (width - L - R);
  const y = (v: number) => T + (1 - (v - f.yMin) / (f.yMax - f.yMin || 1)) * (height - T - B);
  const sec = (iso: string) => Date.parse(`${iso}T00:00:00Z`) / 1000;
  const startNW = $derived(Number(startNetWorth));

  const step = $derived(niceStep(f.yMax - f.yMin, compact ? 3 : 5));
  const ticks = $derived.by(() => {
    const out: number[] = [];
    for (let v = Math.ceil(f.yMin / step) * step; v <= f.yMax; v += step) out.push(v);
    return out;
  });
  const years = $derived.by(() => {
    const y0 = new Date(f.t0 * 1000).getUTCFullYear() + 1, y1 = new Date(f.t1 * 1000).getUTCFullYear();
    const span = y1 - y0;
    const every = span > 24 ? 10 : span > 8 ? 5 : span > 3 ? 2 : 1;
    const out: number[] = [];
    for (let yr = Math.ceil(y0 / every) * every; yr <= y1; yr += every) out.push(yr);
    return out;
  });

  const path = (pts: [number, number][]) => pts.map(([a, b], i) => `${i ? "L" : "M"}${a.toFixed(1)},${b.toFixed(1)}`).join("");
  const band = $derived(path([...f.proj.map((p) => [x(p.t), y(p.hi)] as [number, number]), ...f.proj.slice().reverse().map((p) => [x(p.t), y(p.lo)] as [number, number])]) + "Z");
  const floor = $derived(
    path([[x(f.proj[0].t), y(Math.max(f.yMin, 0))], ...f.proj.map((p) => [x(p.t), y(Math.max(0, startNW) + p.c)] as [number, number]), [x(f.t1), y(Math.max(f.yMin, 0))]]) + "Z",
  );
  const mid = $derived(path(f.proj.map((p) => [x(p.t), y(p.v)])));
  const past = $derived(path(f.hist.map((h) => [x(h.t), y(h.v)])));
  const hit = $derived(target != null ? f.proj.find((p) => p.v >= target) ?? null : null);

  const hovered = $derived.by(() => {
    if (hoverX == null) return null;
    let best = f.proj[0], d = Infinity;
    for (const p of f.proj) { const dd = Math.abs(x(p.t) - hoverX); if (dd < d) { d = dd; best = p; } }
    const h = f.hist.length && hoverX < x(f.proj[0].t) ? f.hist.reduce((a, b) => (Math.abs(x(b.t) - hoverX!) < Math.abs(x(a.t) - hoverX!) ? b : a)) : null;
    return h ? { past: true, t: h.t, v: h.v, lo: h.v, hi: h.v, c: 0 } : { past: false, ...best };
  });
  const iso = (t: number) => new Date(t * 1000).toISOString().slice(0, 10);
  const money = (v: number) => (blur ? "*****" : formatMoney(v, currency));
</script>

<div class="fan" bind:clientWidth={width} style="height:{height}px" class:blur class:compact>
  <svg {width} {height} role="img" aria-label="Net worth so far and projected"
    onpointermove={(e) => (hoverX = e.offsetX)} onpointerleave={() => (hoverX = null)}>
    {#each ticks as v}
      <line x1={L} x2={width - R} y1={y(v)} y2={y(v)} class="grid" />
      <text x={width - R + 8} y={y(v) + 4} class="ax">{blur ? "*****" : formatCompact(v)}</text>
    {/each}
    {#each years as yr}
      <text x={x(sec(`${yr}-01-01`))} y={height - 6} text-anchor="middle" class="ax">{yr}</text>
    {/each}
    {#if !compact}<path d={floor} class="floor" />{/if}
    <path d={band} class="band" />
    <line x1={x(sec(start))} x2={x(sec(start))} y1={T} y2={height - B} class="now" />
    <text x={x(sec(start)) + 5} y={T + 9} class="tag">{compact ? "today · next 12 months dashed" : "today"}</text>
    {#if target != null && target <= f.yMax}
      <line x1={L} x2={width - R} y1={y(target)} y2={y(target)} class="target" />
    {/if}
    <path d={past} class="past" />
    <path d={mid} class="mid" />
    {#if !compact}
      {#each debts.filter((d) => d.payoff && sec(d.payoff) <= f.t1) as d}
        <line x1={x(sec(d.payoff!))} x2={x(sec(d.payoff!))} y1={y(Math.max(f.yMin, 0)) - 12} y2={y(Math.max(f.yMin, 0))} class="payoff" />
        <text x={x(sec(d.payoff!))} y={y(Math.max(f.yMin, 0)) - 16} text-anchor="middle" class="tag">{d.name} paid off</text>
      {/each}
      {#each milestones as m}
        <circle cx={x(sec(m.date))} cy={y(Number(m.net_worth))} r="3" class="dot" />
        <text x={x(sec(m.date))} y={y(Number(m.net_worth)) + 16} text-anchor="middle" class="tag">{m.years}y</text>
      {/each}
    {/if}
    {#if hit && target != null}
      <circle cx={x(hit.t)} cy={y(target)} r="4.5" class="hit" />
      {#if !compact}<text x={x(hit.t) - 8} y={y(target) - 8} text-anchor="end" class="tag">{blur ? "target" : formatCompact(target)} · {monthYear(iso(hit.t))}</text>{/if}
    {/if}
    {#if hovered}
      <line x1={x(hovered.t)} x2={x(hovered.t)} y1={T} y2={height - B} class="cursor" />
      <circle cx={x(hovered.t)} cy={y(hovered.v)} r="3.5" class="dot" />
    {/if}
  </svg>
  {#if hovered}
    <div class="tip" style="left:{x(hovered.t) > width / 2 ? x(hovered.t) - 196 : x(hovered.t) + 14}px">
      <div class="top"><b class="money">{money(hovered.v)}</b><span>{monthYear(iso(hovered.t))}</span></div>
      {#if !hovered.past}
        <div class="row"><span>range</span><span class="money">{blur ? "*****" : `${formatCompact(hovered.lo)}–${formatCompact(hovered.hi)}`}</span></div>
        <div class="row"><span>put in by then</span><span class="money">{money(hovered.c)}</span></div>
        <div class="row"><span>growth and paydown</span><span class="money">{money(hovered.v - startNW - hovered.c)}</span></div>
      {:else}
        <div class="row"><span>recorded</span><span></span></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .fan { position: relative; width: 100%; }
  svg { display: block; overflow: visible; touch-action: pan-y; }
  .grid { stroke: var(--border); }
  .ax { font-family: var(--figures); font-size: 11px; fill: var(--text-3); }
  .tag { font-family: var(--font); font-size: 11px; fill: var(--text-2); }
  .band { fill: var(--accent-soft); }
  .floor { fill: var(--fill-soft); opacity: 0.45; }
  .past { fill: none; stroke: var(--accent); stroke-width: 2; }
  .compact .past { stroke-width: 3; }
  .compact .mid { stroke-width: 2.2; }
  .mid { fill: none; stroke: var(--accent); stroke-width: 1.6; stroke-dasharray: 5 4; }
  .now { stroke: var(--text-3); stroke-dasharray: 2 3; }
  .target { stroke: var(--text-2); stroke-dasharray: 1 3; }
  .payoff { stroke: var(--g-debt); stroke-width: 2; }
  .dot { fill: var(--accent); }
  .hit { fill: var(--bg); stroke: var(--accent); stroke-width: 1.5; }
  .cursor { stroke: var(--text-3); }
  .tip { position: absolute; top: 8px; width: 182px; background: var(--surface-2); border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px; font-size: 12px; pointer-events: none; }
  .tip .top { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 4px; }
  .tip .top span, .tip .row span:first-child { color: var(--text-3); }
  .tip .row { display: flex; justify-content: space-between; }
  .tip b { font-weight: 500; }
</style>
