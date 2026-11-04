<script lang="ts">
  // Where one company's dollars come from: one band per fund path, as thick as the dollars it
  // carries, all running into the company on the right. Labels stay text, so they read and copy.
  import type { Via } from "../../api";
  import { formatMoney, formatPct } from "../../format/money";

  let { via, name, color = "var(--fill)", currency = "CAD", label }: { via: Via[]; name: string; color?: string; currency?: string; label: (v: Via) => string } = $props();
  let w = $state(300);
  const ROW = 34, PAD = 6, SINK = 8;
  const total = $derived(via.reduce((s, v) => s + Number(v.value), 0) || 1);
  const h = $derived(Math.max(via.length * ROW, 60));
  const sinkH = $derived(Math.min(h - 2 * PAD, Math.max(24, h * 0.7)));
  const sinkY = $derived((h - sinkH) / 2);
  const x0 = $derived(Math.min(150, w * 0.5));
  const x1 = $derived(w - SINK - 2);
  const bands = $derived.by(() => {
    let acc = sinkY;
    return via.map((v, i) => {
      const t = Math.max(2, (Number(v.value) / total) * sinkH);
      const y0 = i * ROW + ROW / 2, y1 = acc + t / 2;
      acc += t;
      // A ribbon that fans out from a thin start at the label to its share of the company bar.
      const mx = (x0 + x1) / 2, l = 1.5, r = t / 2;
      return { v, t, y0, d: `M${x0} ${y0 - l} C ${mx} ${y0 - l}, ${mx} ${y1 - r}, ${x1} ${y1 - r} L ${x1} ${y1 + r} C ${mx} ${y1 + r}, ${mx} ${y0 + l}, ${x0} ${y0 + l} Z` };
    });
  });
</script>

<div class="lin" bind:clientWidth={w}>
  <svg width={w} height={h} role="img" aria-label="{name} through {via.map((v) => `${label(v)} ${formatMoney(v.value, currency)}`).join(', ')}">
    {#each bands as b (b.v.path.join(">"))}
      <path d={b.d} fill={color} />
      <text x="0" y={b.y0 - 2} class="l">{label(b.v)}</text>
      <text x="0" y={b.y0 + 12} class="s">{formatPct(b.v.weight_pct, 2)} of {b.v.path[0]}</text>
      <text x={x0 - 6} y={b.y0 + 12} class="v money" text-anchor="end">{formatMoney(b.v.value, currency)}</text>
    {/each}
    <rect x={x1} y={sinkY} width={SINK} height={sinkH} fill={color} rx="2" />
  </svg>
</div>

<style>
  .lin { width: 100%; margin-top: 12px; }
  svg { display: block; overflow: visible; }
  path { fill-opacity: 0.5; }
  .l { font-size: 12px; fill: var(--text); }
  .s { font-size: 10px; fill: var(--text-3); }
  .v { font-family: var(--figures); font-size: 11px; fill: var(--text-2); }
  @media (prefers-reduced-motion: no-preference) {
    path { animation: flow 560ms var(--ease-out) both; }
  }
  @keyframes flow { from { clip-path: inset(0 100% 0 0); } to { clip-path: inset(0 0 0 0); } }
</style>
