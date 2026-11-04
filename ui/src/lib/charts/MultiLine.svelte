<script lang="ts">
  // Several dated series on one scale, each labelled at its right end: funds' returns
  // since the start of a period, so they compare at a glance. Hover reads every line on a day.
  type Series = { label: string; color: string; pts: { date: string; v: number }[] };
  let { series, format, height = 240, label }: { series: Series[]; format: (v: number) => string; height?: number; label: string } = $props();
  let w = $state(600);
  let at = $state<string | null>(null);
  const PAD = { l: 4, r: 116, t: 12, b: 22 };
  const all = $derived(series.flatMap((s) => s.pts));
  const lo = $derived(Math.min(0, ...all.map((p) => p.v)));
  const hi = $derived(Math.max(0, ...all.map((p) => p.v)));
  const span = $derived(hi - lo || 1);
  const t0 = $derived(all.length ? Math.min(...all.map((p) => new Date(p.date).getTime())) : 0);
  const t1 = $derived(all.length ? Math.max(...all.map((p) => new Date(p.date).getTime())) : 1);
  const x = (d: string) => PAD.l + ((new Date(d).getTime() - t0) / Math.max(1, t1 - t0)) * (w - PAD.l - PAD.r);
  const y = (v: number) => PAD.t + (1 - (v - lo) / span) * (height - PAD.t - PAD.b);
  const path = (pts: Series["pts"]) => pts.map((p, i) => `${i ? "L" : "M"}${x(p.date)},${y(p.v)}`).join("");
  // End labels, nudged apart so they never overlap.
  const ends = $derived.by(() => {
    const e = series.filter((s) => s.pts.length).map((s) => ({ s, y: y(s.pts[s.pts.length - 1].v) })).sort((a, b) => a.y - b.y);
    for (let i = 1; i < e.length; i++) if (e[i].y - e[i - 1].y < 14) e[i].y = e[i - 1].y + 14;
    return e;
  });
  const dates = $derived([...new Set(all.map((p) => p.date))].sort());
  function move(ev: PointerEvent) {
    const r = (ev.currentTarget as SVGElement).getBoundingClientRect();
    const px = ev.clientX - r.left;
    let best = dates[0];
    for (const d of dates) if (Math.abs(x(d) - px) < Math.abs(x(best) - px)) best = d;
    at = best ?? null;
  }
  const fmtDate = (d: string) => new Date(d + "T12:00:00").toLocaleDateString("en-CA", { month: "short", day: "numeric" });
</script>

<div class="ml" bind:clientWidth={w}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <svg width={w} {height} role="img" aria-label="{label}: {series.map((s) => `${s.label} ${s.pts.length ? format(s.pts[s.pts.length - 1].v) : ''}`).join(', ')}" onpointermove={move} onpointerleave={() => (at = null)}>
    <line class="zero" x1={PAD.l} x2={w - PAD.r} y1={y(0)} y2={y(0)} />
    {#each series as s (s.label)}<path d={path(s.pts)} style="stroke:{s.color}" />{/each}
    {#each ends as e (e.s.label)}
      <text class="lab" x={w - PAD.r + 8} y={e.y + 4} style="fill:{e.s.color}">{e.s.label} <tspan class="num">{format(e.s.pts[e.s.pts.length - 1].v)}</tspan></text>
    {/each}
    {#if dates.length}
      <text class="end" x={PAD.l} y={height - 6}>{fmtDate(dates[0])}</text>
      <text class="end" x={w - PAD.r} y={height - 6} text-anchor="end">{fmtDate(dates[dates.length - 1])}</text>
    {/if}
    {#if at}
      <line class="cross" x1={x(at)} x2={x(at)} y1={PAD.t} y2={height - PAD.b} />
      {#each series as s (s.label)}{@const p = s.pts.find((q) => q.date === at)}{#if p}<circle cx={x(p.date)} cy={y(p.v)} r="3" style="fill:{s.color}" />{/if}{/each}
    {/if}
  </svg>
  {#if at}<div class="read">{fmtDate(at)} · {series.map((s) => { const p = s.pts.find((q) => q.date === at); return p ? `${s.label} ${format(p.v)}` : ""; }).filter(Boolean).join(" · ")}</div>{/if}
</div>

<style>
  /* contain: inline-size: the measured SVG never props its parent open. */
  .ml { position: relative; width: 100%; contain: inline-size; }
  svg { display: block; touch-action: pan-y; }
  path { fill: none; stroke-width: 1.8; stroke-linejoin: round; }
  .zero { stroke: var(--border); stroke-dasharray: 3 3; }
  .cross { stroke: var(--border); }
  .lab { font-size: 12px; font-family: var(--font); font-weight: 500; }
  .end { font-size: 11px; fill: var(--text-3); font-family: var(--font); }
  .read { position: absolute; top: -18px; left: 0; font-size: 12px; color: var(--text-2); }
</style>
