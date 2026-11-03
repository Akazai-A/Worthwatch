<script lang="ts">
  // A constituent's weight in a fund, before and after: a thin track from 0 to the
  // column's largest weight, the old weight as a mark, the move as a coloured span.
  let { from, to, max, width = 110 }: { from: number; to: number; max: number; width?: number } = $props();
  const x = (v: number) => (max > 0 ? Math.min(100, (v / max) * 100) : 0);
  const lo = $derived(Math.min(from, to));
  const hi = $derived(Math.max(from, to));
</script>

<span class="wm" style="width:{width}px" aria-hidden="true">
  <i class="base" style="width:{x(lo)}%"></i>
  <i class:up={to > from} class:down={to < from} style="left:{x(lo)}%;width:{Math.max(1, x(hi) - x(lo))}%"></i>
  <b style="left:{x(from)}%"></b>
</span>

<style>
  .wm { position: relative; display: inline-block; height: 8px; background: var(--surface-2); border-radius: 3px; overflow: hidden; vertical-align: middle; }
  i { position: absolute; top: 0; bottom: 0; }
  .base { left: 0; background: var(--fill-soft); }
  .up { background: var(--positive); }
  .down { background: var(--negative); }
  b { position: absolute; top: 0; bottom: 0; width: 1px; margin-left: -0.5px; background: var(--text); }
</style>
