<script lang="ts">
  // One dated series as a line over a soft area, drawn at the width it is given. Hover or
  // arrow keys move a readout along it; the first and last values label the ends.
  // `step` draws a staircase, for values that hold between check-ins; `zero` starts the
  // scale at zero, for amounts you own, so a small move doesn't look like a cliff.
  let {
    points,
    label,
    format,
    height = 220,
    tone = "neutral",
    step = false,
    zero = false,
    bare = false,
  }: { points: { date: string; v: number }[]; label: string; format: (v: number) => string; height?: number; tone?: "up" | "down" | "neutral"; step?: boolean; zero?: boolean;
    /** A sparkline: no date labels and no readout. */
    bare?: boolean } = $props();
  let w = $state(600);
  let at = $state<number | null>(null);
  const PAD = $derived(bare ? { l: 1, r: 1, t: 3, b: 3 } : { l: 4, r: 4, t: 16, b: 22 });
  const lo = $derived(Math.min(zero ? 0 : Infinity, ...points.map((p) => p.v)));
  const hi = $derived(Math.max(...points.map((p) => p.v)));
  const span = $derived(hi - lo || Math.abs(hi) || 1);
  const t0 = $derived(points.length ? new Date(points[0].date).getTime() : 0);
  const t1 = $derived(points.length ? new Date(points[points.length - 1].date).getTime() : 1);
  const x = (d: string) => PAD.l + ((new Date(d).getTime() - t0) / Math.max(1, t1 - t0)) * (w - PAD.l - PAD.r);
  const y = (v: number) => PAD.t + (1 - (v - lo + (zero ? 0 : span * 0.08)) / (span * (zero ? 1.08 : 1.16))) * (height - PAD.t - PAD.b);
  const line = $derived(
    points.map((p, i) => {
      const X = x(p.date), Y = y(p.v);
      if (i === 0) return `M${X},${Y}`;
      return step ? `H${X}V${Y}` : `L${X},${Y}`;
    }).join(""),
  );
  const area = $derived(points.length ? `${line}V${height - PAD.b}H${x(points[0].date)}Z` : "");
  const fmtDate = (d: string) => new Date(d + "T12:00:00").toLocaleDateString("en-CA", { month: "short", day: "numeric", year: "2-digit" });
  function move(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = e.clientX - r.left;
    let best = 0;
    for (let i = 0; i < points.length; i++) if (Math.abs(x(points[i].date) - px) < Math.abs(x(points[best].date) - px)) best = i;
    at = best;
  }
  function key(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") at = Math.max(0, (at ?? points.length) - 1);
    else if (e.key === "ArrowRight") at = Math.min(points.length - 1, (at ?? -1) + 1);
    else return;
    e.preventDefault();
  }
  const cur = $derived(at != null ? points[at] : null);
</script>

<div class="lc" bind:clientWidth={w}>
  {#if points.length > 1}
    <!-- Arrow keys walk the readout along the line; a pointer does the same. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <svg width={w} {height} role="img" aria-label="{label}: {format(points[0].v)} on {fmtDate(points[0].date)} to {format(points[points.length - 1].v)} on {fmtDate(points[points.length - 1].date)}" tabindex="0" onpointermove={move} onpointerleave={() => (at = null)} onkeydown={key} onblur={() => (at = null)}>
      <path class="area {tone}" d={area} />
      <path class="line {tone}" d={line} />
      {#if !bare}
        <text class="end" x={PAD.l} y={height - 6}>{fmtDate(points[0].date)}</text>
        <text class="end" x={w - PAD.r} y={height - 6} text-anchor="end">{fmtDate(points[points.length - 1].date)}</text>
      {/if}
      {#if cur}
        <line class="cross" x1={x(cur.date)} x2={x(cur.date)} y1={PAD.t - 6} y2={height - PAD.b} />
        <circle class="dot {tone}" cx={x(cur.date)} cy={y(cur.v)} r="3.5" />
      {/if}
    </svg>
    <div class="read num" class:bare aria-live="polite">{#if cur}<b>{format(cur.v)}</b> <span class="faint">{fmtDate(cur.date)}</span>{/if}</div>
  {:else if points.length === 1}
    <p class="faint one">One point so far: <span>{format(points[0].v)}</span> on {fmtDate(points[0].date)}.</p>
  {/if}
</div>

<style>
  /* contain: inline-size: the measured SVG never props its parent open, so it shrinks with the window. */
  .lc { position: relative; width: 100%; contain: inline-size; }
  svg { display: block; touch-action: pan-y; cursor: crosshair; }
  svg:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 4px; }
  .line { fill: none; stroke: var(--text-2); stroke-width: 1.6; stroke-linejoin: round; }
  .line.up { stroke: var(--positive); }
  .line.down { stroke: var(--negative); }
  .area { fill: var(--text); opacity: 0.06; }
  .area.up { fill: var(--positive); opacity: 0.1; }
  .area.down { fill: var(--negative); opacity: 0.1; }
  .end { font-size: 11px; fill: var(--text-3); font-family: var(--font); }
  .cross { stroke: var(--border); stroke-width: 1; }
  .dot { fill: var(--text); }
  .dot.up { fill: var(--positive); }
  .dot.down { fill: var(--negative); }
  .read { position: absolute; top: 0; right: 4px; font-size: 12px; pointer-events: none; }
  .read b { font-weight: 500; }
  .read.bare { top: auto; bottom: 100%; right: 0; font-size: 11px; }
  .one { margin: 8px 0; font-size: 13px; }
</style>
