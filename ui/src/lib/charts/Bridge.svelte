<script lang="ts">
  // Waterfall from the starting net worth to the ending one, one floating bar per
  // step. Plain SVG; the x range fits the path so small steps stay visible.
  import type { BridgeStep } from "../changes";
  import { formatCompact, formatMoney, formatSigned } from "../money";

  let {
    start,
    end,
    steps,
    startLabel,
    endLabel,
    currency = "CAD",
    blur = false,
  }: { start: number; end: number; steps: BridgeStep[]; startLabel: string; endLabel: string; currency?: string; blur?: boolean } = $props();

  const ROW = 19;
  const GUTTER = 130;
  const RIGHT = 90;
  const W = 640;
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
  const money = (v: number) => (blur ? "*****" : formatMoney(v, currency));
  const signed = (v: number) => (blur ? "*****" : formatSigned(v, currency));
</script>

<svg class="bridge" viewBox="0 0 {W} {height}" width="100%" style="height:{height}px;min-width:460px" role="img" aria-label="Net worth bridge from {startLabel} to {endLabel}">
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
    <g transform="translate(0,{y})">
      <text class="lab" class:faint={r.folded} x={GUTTER - 10} y={4} text-anchor="end">{r.label}</text>
      <rect class:up={r.delta > 0} class:down={r.delta < 0} x={left} y={-6} width={width} height={12} rx="1.5" />
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

<style>
  .bridge { display: block; overflow: visible; }
  .axis { stroke: var(--border); stroke-width: 1; }
  .lab { font-family: var(--font); font-size: 12px; fill: var(--text-2); }
  .lab.faint { fill: var(--text-3); }
  .val { font-family: var(--figures); font-size: 11px; fill: var(--text-2); }
  .tick { font-family: var(--figures); font-size: 10px; fill: var(--text-3); }
  rect { fill: var(--fill); }
  rect.up { fill: var(--positive); }
  rect.down { fill: var(--negative); }
  text.up { fill: var(--positive); }
  text.down { fill: var(--negative); }
  .mark { fill: var(--text-3); }
  .mark.end { fill: var(--accent); }
</style>
