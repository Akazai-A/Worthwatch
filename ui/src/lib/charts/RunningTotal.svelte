<script lang="ts">
  // Running total for the month: confirmed spending solid, taps not on a statement yet
  // dashed above it, the budget pace (bills on their day, the rest evenly), and each day's spending as a strip
  // of bars on its own scale underneath. Hover a day for its numbers.
  import type { BudgetView } from "../api";
  import { chart } from "../budget";
  import { formatCompact, formatMoney } from "../money";
  import { prefs } from "../stores/prefs.svelte";

  let { v }: { v: BudgetView } = $props();
  const W = 780, H = 236, STRIP = 34;
  const c = $derived(chart(v, W, H, STRIP));
  const money = (n: number) => (prefs.blur ? "*****" : formatMoney(n, v.base_currency));
  const compact = (n: number) => (prefs.blur ? "*****" : formatCompact(n));
  let hover = $state<number | null>(null);
  const stripTop = H - STRIP - 18;
  const monthShort = $derived(new Intl.DateTimeFormat("en-CA", { month: "short", timeZone: "UTC" }).format(new Date(v.month + "T00:00:00Z")));
  const ticks = $derived([1, 8, 15, 22, 29].filter((d) => d <= v.days));

  function onMove(e: PointerEvent) {
    const svg = e.currentTarget as SVGSVGElement;
    const r = svg.getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    const day = Math.round(((px - c.x(0)) / (c.x(v.days) - c.x(0))) * v.days + 0.5);
    hover = day >= 1 && day <= v.days ? day : null;
  }
  const hd = $derived(hover != null ? { day: hover, daily: v.daily[hover - 1], cum: v.cumulative[hover - 1], prov: v.cumulative_provisional[hover - 1] } : null);
</script>

<div class="wrap">
  <svg viewBox="0 0 {W} {H}" width="100%" role="img" aria-label="Running total of spending this month against an even pace to the budget"
    onpointermove={onMove} onpointerleave={() => (hover = null)}>
    {#each c.yTicks as t}
      <line class="grid" x1={c.x(0)} x2={c.x(v.days)} y1={t.y} y2={t.y} />
      <text class="ax" x={c.x(v.days) + 8} y={t.y + 3}>{compact(t.value)}</text>
    {/each}
    {#if c.pace}<path class="pace" d={c.pace} />
      <text class="ax" x={c.x(v.days) - 4} y={c.y(Number(v.budget)) - 8} text-anchor="end">budget {compact(Number(v.budget))}</text>{/if}
    <path class="conf" d={c.confirmed} />
    {#if c.total}<path class="prov" d={c.total} />{/if}
    {#if c.today && v.day < v.days}
      <line class="now" x1={c.today.x} x2={c.today.x} y1="12" y2={stripTop + STRIP} />
    {/if}
    {#if c.today}<circle class="dot" cx={c.today.x} cy={c.today.y} r="4.5" />{/if}
    {#each c.bars as b}
      <rect class="bar" class:hot={hover === b.day} x={b.x} y={stripTop + STRIP - b.h} width={b.w} height={b.h} rx="2" />
      {#if b.clipped}<line class="clip" x1={b.x - 1} x2={b.x + b.w + 1} y1={stripTop + STRIP - b.h + 5} y2={stripTop + STRIP - b.h + 2} />{/if}
    {/each}
    <text class="ax" x={c.x(v.days) + 8} y={stripTop + STRIP - 4}>per day</text>
    {#each ticks as d}
      <text class="ax" x={c.x(d - 0.5)} y={H - 4} text-anchor="middle">{monthShort} {d}</text>
    {/each}
    {#if hd}
      <line class="hover" x1={c.x(hd.day - 0.5)} x2={c.x(hd.day - 0.5)} y1="12" y2={stripTop + STRIP} />
    {/if}
  </svg>
  {#if hd}
    <div class="tip" style="left:{Math.min(78, (c.x(hd.day - 0.5) / W) * 100)}%">
      <b>{monthShort} {hd.day}</b>
      {#if hd.cum == null}<span class="faint">still to come</span>
      {:else}
        <span>spent that day <b class="money">{money(Number(hd.daily))}</b></span>
        <span>month so far <b class="money">{money(Number(hd.cum))}</b></span>
        {#if Number(hd.prov) > 0}<span class="faint">{money(Number(hd.prov))} of it taps, not on a statement yet</span>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; }
  svg { display: block; touch-action: pan-y; }
  .grid { stroke: var(--border); }
  .ax { font-size: 10px; fill: var(--text-3); font-family: var(--figures); }
  .pace { stroke: var(--text-3); stroke-dasharray: 3 4; fill: none; }
  .conf { stroke: var(--accent); stroke-width: 2; fill: none; stroke-linejoin: round; }
  .prov { stroke: var(--accent); stroke-width: 2; fill: none; stroke-dasharray: 5 4; }
  .dot { fill: var(--accent); stroke: var(--surface); stroke-width: 2; }
  .now { stroke: var(--text-3); }
  .bar { fill: var(--fill); opacity: 0.6; }
  .bar.hot { opacity: 1; }
  .clip { stroke: var(--surface); stroke-width: 2; }
  .hover { stroke: var(--text-2); stroke-width: 1; pointer-events: none; }
  .tip { position: absolute; top: 4px; transform: translateX(8px); background: var(--surface-2); border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px; font-size: 12px; display: flex; flex-direction: column; gap: 2px; pointer-events: none; white-space: nowrap; }
</style>
