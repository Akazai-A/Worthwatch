<script lang="ts">
  // Tiny SVG sparkline: last N points, endpoint in the accent.
  let { values, width = 90, height = 24 }: { values: number[]; width?: number; height?: number } = $props();
  const geometry = $derived.by(() => {
    if (values.length < 2) return null;
    const lo = Math.min(...values), hi = Math.max(...values);
    const span = hi - lo || 1;
    const x = (i: number) => 2 + (i * (width - 4)) / (values.length - 1);
    const y = (v: number) => height - 4 - ((v - lo) * (height - 8)) / span;
    const d = values.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)} ${y(v).toFixed(1)}`).join(" ");
    return { d, ex: x(values.length - 1), ey: y(values[values.length - 1]) };
  });
</script>

{#if geometry}
  <svg class="spark" viewBox="0 0 {width} {height}" width={width} height={height} aria-hidden="true">
    <path d={geometry.d} />
    <circle cx={geometry.ex} cy={geometry.ey} r="2.5" />
  </svg>
{:else}
  <span class="spark empty"></span>
{/if}

<style>
  .spark { display: block; }
  path { fill: none; stroke: var(--text-3); stroke-width: 1.5; stroke-linejoin: round; stroke-linecap: round; }
  circle { fill: var(--accent); }
  .empty { display: inline-block; }
</style>
