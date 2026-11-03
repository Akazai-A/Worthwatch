<script lang="ts">
  // Waterfall from the starting net worth to the ending one, one floating bar per
  // step. Plain SVG; the x range fits the path so small steps stay visible.
  import type { Snippet } from "svelte";
  import type { BridgeStep } from "../portfolio/changes";
  import { formatCompact, formatMoney, formatSigned } from "../format/money";

  let {
    start,
    end,
    steps,
    startLabel,
    endLabel,
    currency = "CAD",
    blur = false,
    tip,
    onunfold,
    title = "Net worth",
  }: {
    /** What the bridge walks from start to end, for screen readers. */
    title?: string;
    start: number; end: number; steps: BridgeStep[]; startLabel: string; endLabel: string; currency?: string; blur?: boolean;
    /** Card shown while a step is hovered or focused: from → to and what made it. */
    tip?: Snippet<[BridgeStep]>;
    /** Click on the folded "N others" step. */
    onunfold?: () => void;
  } = $props();
  let active = $state<number | null>(null);

  const ROW = 22;
  // Drawn at the width it is given, so text stays at its real size on a wide page,
  // with a narrower label gutter on a phone so the step values stay on screen.
  let cw = $state(640);
  const W = $derived(Math.max(300, cw));
  const GUTTER = $derived(W < 520 ? 104 : 150);
  const RIGHT = $derived(W < 520 ? 72 : 96);
  const rows = $derived.by(() => {
    let cum = start;
    const out = steps.map((s) => {
      const from = cum;
      cum += s.delta;
      return { ...s, from, to: cum };
    });
    return out;
  });
  const height = $derived(ROW * (rows.length + 2) + 8);
  const domain = $derived.by(() => {
    const vals = [start, end, ...rows.flatMap((r) => [r.from, r.to])];
    let lo = Math.min(...vals), hi = Math.max(...vals);
    const span = hi - lo || Math.abs(hi) || 1;
    lo -= span * 0.06;
    hi += span * 0.06;
    return { lo, hi };
  });
  const x = (v: number) => GUTTER + ((v - domain.lo) / (domain.hi - domain.lo)) * (W - GUTTER - RIGHT);
  const short = (t: string) => (W < 520 && t.length > 13 ? t.slice(0, 12).trimEnd() + "…" : t);
  const money = (v: number) => (blur ? "*****" : formatMoney(v, currency));
  const signed = (v: number) => (blur ? "*****" : formatSigned(v, currency));
</script>

<div class="fit" bind:clientWidth={cw}>
{#if tip && active != null && rows[active] && !rows[active].folded}
  {@const r = rows[active]}
  {@const right = x(Math.max(r.from, r.to))}
  <div class="tip" style="top:{ROW * (active + 1) + ROW + 6}px;{right > W * 0.6 ? `right:${W - x(Math.min(r.from, r.to)) + 8}px` : `left:${right + 70}px`}">{@render tip(r)}</div>
{/if}
<svg class="bridge" viewBox="0 0 {W} {height}" width={W} height={height} role="group" aria-label="{title} bridge from {startLabel} to {endLabel}">
  <line class="axis" x1={x(start)} x2={x(start)} y1={4} y2={height - 4} />
  <g transform="translate(0,{ROW / 2})">
    <text class="lab" x={GUTTER - 10} y={4} text-anchor="end">{startLabel}</text>
    <circle class="mark" cx={x(start)} cy={0} r="3.5" />
    <text class="val" x={x(start) + 8} y={4}>{money(start)}</text>
  </g>
  {#each rows as r, i}
    {@const y = ROW * (i + 1) + ROW / 2}
    {@const left = x(Math.min(r.from, r.to))}
    {@const width = Math.max(1.5, Math.abs(x(r.to) - x(r.from)))}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <g
      class="step" class:fold={r.folded && onunfold} transform="translate(0,{y})" tabindex={tip || (r.folded && onunfold) ? 0 : undefined}
      role={r.folded && onunfold ? "button" : "listitem"} aria-label="{r.label} {signed(r.delta)}{r.folded && onunfold ? ', show each' : ''}"
      onmouseenter={() => (active = i)} onmouseleave={() => (active = null)} onfocus={() => (active = i)} onblur={() => (active = null)}
      onclick={() => r.folded && onunfold?.()} onkeydown={(e) => { if (r.folded && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); onunfold?.(); } }}
    >
      <rect class="hit" x={0} y={-ROW / 2} width={W} height={ROW} />
      <text class="lab" class:faint={r.folded} x={GUTTER - 10} y={4} text-anchor="end">{short(r.label)}{r.folded && onunfold ? " ▸" : ""}</text>
      <rect class="c-{r.cause ?? 'none'}" class:up={r.delta > 0} class:down={r.delta < 0} style="--i:{Math.min(i, 10)}" x={left} y={-6} width={width} height={12} rx="1.5" />
      <text class="val" class:up={r.delta > 0} class:down={r.delta < 0} x={left + width + 6} y={4}>{signed(r.delta)}</text>
    </g>
  {/each}
  <g transform="translate(0,{ROW * (rows.length + 1) + ROW / 2})">
    <text class="lab" x={GUTTER - 10} y={4} text-anchor="end">{endLabel}</text>
    <circle class="mark end" cx={x(end)} cy={0} r="3.5" />
    <text class="val" x={x(end) + 8} y={4}>{money(end)}</text>
  </g>
  <text class="tick" x={W - RIGHT} y={height - 2} text-anchor="end">{blur ? "" : `${formatCompact(domain.lo)} – ${formatCompact(domain.hi)}`}</text>
</svg>
</div>

<style>
  .fit { position: relative; contain: inline-size; }
  .bridge { display: block; overflow: visible; }
  .step { outline: none; }
  .step .hit { fill: transparent; }
  .step:hover .hit, .step:focus-visible .hit { fill: var(--surface-2); }
  .step:focus-visible .lab { fill: var(--accent); }
  .step.fold { cursor: pointer; }
  .tip { position: absolute; z-index: 3; width: 300px; background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 10px 12px; font-size: 12px; box-shadow: 0 8px 28px rgb(0 0 0 / 0.3); pointer-events: none; }
  .axis { stroke: var(--border); stroke-width: 1; }
  .lab { font-family: var(--font); font-size: 12px; fill: var(--text-2); }
  .lab.faint { fill: var(--text-3); }
  .val { font-family: var(--figures); font-size: 11px; fill: var(--text-2); }
  .tick { font-family: var(--figures); font-size: 10px; fill: var(--text-3); }
  rect:not(.hit) { fill: var(--fill); }
  rect.up { fill: var(--positive); }
  rect.down { fill: var(--negative); }
  /* Steps take the colour of their cause: market (sage, clay when it fell), money you moved (bone), debt (violet). */
  rect.c-money { fill: var(--fill); }
  rect.c-debt { fill: var(--g-debt); }
  rect.c-debt.down { fill: color-mix(in oklab, var(--g-debt) 55%, var(--negative)); }
  text.up { fill: var(--positive); }
  text.down { fill: var(--negative); }
  /* The waterfall builds a step at a time, each bar growing from where the last one ended. */
  @media (prefers-reduced-motion: no-preference) {
    rect.up, rect.down { transform-box: fill-box; animation: grow 480ms var(--ease-out) both; animation-delay: calc(60ms + var(--i) * 45ms); }
    rect.up { transform-origin: left center; }
    rect.down { transform-origin: right center; }
  }
  @keyframes grow { from { transform: scaleX(0); } }
  .mark { fill: var(--text-3); }
  .mark.end { fill: var(--accent); }
</style>
