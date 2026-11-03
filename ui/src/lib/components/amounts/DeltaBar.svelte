<script lang="ts">
  // A diverging bar: zero in the middle, green to the right, red to the left,
  // scaled to the largest value in its column so rows compare at a glance.
  let { value, max, width = 72 }: { value: number; max: number; width?: number } = $props();
  const pct = $derived(max > 0 ? Math.min(50, (Math.abs(value) / max) * 50) : 0);
</script>

<span class="bar" style="width:{width}px" aria-hidden="true">
  {#if value > 0}<i class="up" style="left:50%;width:{pct}%"></i>{:else if value < 0}<i class="down" style="right:50%;width:{pct}%"></i>{/if}
</span>

<style>
  .bar { position: relative; display: inline-block; height: 8px; background: var(--surface-2); border-radius: 3px; overflow: hidden; vertical-align: middle; }
  .bar::before { content: ""; position: absolute; left: 50%; top: 0; bottom: 0; width: 1px; background: var(--border); }
  i { position: absolute; top: 0; bottom: 0; min-width: 1px; }
  .up { background: var(--positive); border-radius: 0 3px 3px 0; }
  .down { background: var(--negative); border-radius: 3px 0 0 3px; }
</style>
