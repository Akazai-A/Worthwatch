<script lang="ts">
  // Parts of one whole as a ring, in the order given, from twelve o'clock. Hover a slice for its
  // share; otherwise the hole shows `center`.
  let { slices, size = 120, hole = 0.6, label, center = null }: {
    slices: { label: string; value: number; color: string }[]; size?: number; hole?: number; label: string;
    /** Text in the hole when nothing is hovered: [figure, caption]. */
    center?: [string, string] | null;
  } = $props();
  let on = $state<number | null>(null);
  const total = $derived(slices.reduce((s, x) => s + Math.max(0, x.value), 0) || 1);
  const C = $derived(size / 2);
  const R = $derived(size / 2), r = $derived(R * hole);
  const arcs = $derived.by(() => {
    let a = -Math.PI / 2;
    return slices.map((s) => {
      const sweep = (Math.max(0, s.value) / total) * Math.PI * 2;
      const a0 = a, a1 = a + sweep;
      a = a1;
      const big = sweep > Math.PI ? 1 : 0;
      const p = (rad: number, ang: number) => `${C + rad * Math.cos(ang)},${C + rad * Math.sin(ang)}`;
      const d = sweep >= Math.PI * 2 - 1e-6
        ? `M${p(R, 0)}A${R},${R} 0 1 1 ${p(R, Math.PI)}A${R},${R} 0 1 1 ${p(R, 0)}M${p(r, 0)}A${r},${r} 0 1 0 ${p(r, Math.PI)}A${r},${r} 0 1 0 ${p(r, 0)}Z`
        : `M${p(R, a0)}A${R},${R} 0 ${big} 1 ${p(R, a1)}L${p(r, a1)}A${r},${r} 0 ${big} 0 ${p(r, a0)}Z`;
      return { s, d, pct: (Math.max(0, s.value) / total) * 100 };
    });
  });
</script>

<svg width={size} height={size} viewBox="0 0 {size} {size}" role="img" aria-label="{label}: {arcs.map((x) => `${x.s.label} ${x.pct.toFixed(0)}%`).join(', ')}">
  {#each arcs as x, i (x.s.label)}
    <path d={x.d} style="fill:{x.s.color}" class:dim={on != null && on !== i} onpointerenter={() => (on = i)} onpointerleave={() => (on = null)} role="presentation"><title>{x.s.label} {x.pct.toFixed(1)}%</title></path>
  {/each}
  {#if on != null && hole > 0}
    <text x={C} y={C - 2} text-anchor="middle" class="v">{arcs[on].pct.toFixed(0)}%</text>
    <text x={C} y={C + 12} text-anchor="middle" class="l">{arcs[on].s.label.split(" ")[0].replace(",", "")}</text>
  {:else if center && hole > 0}
    <text x={C} y={C - 2} text-anchor="middle" class="v">{center[0]}</text>
    <text x={C} y={C + 12} text-anchor="middle" class="l">{center[1]}</text>
  {/if}
</svg>

<style>
  svg { display: block; flex: none; }
  path { stroke: var(--bg); stroke-width: 1.5; transition: opacity 120ms; }
  path.dim { opacity: 0.35; }
  .v { font-family: var(--figures); font-size: 15px; fill: var(--text); }
  .l { font-size: 10px; fill: var(--text-3); font-family: var(--font); }
</style>
