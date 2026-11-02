<script lang="ts">
  // A stacked strip of signed parts (market, trades, FX, ...). Each part is a
  // segment sized by its absolute value, green or red by sign, dimmed by rank so
  // the parts stay distinguishable without a second hue; the legend names them.
  import { formatSigned } from "../money";
  export interface Part { label: string; value: number }
  let {
    parts,
    currency = "CAD",
    width = "100%",
    height = 8,
    scale = null,
    legend = false,
    blur = false,
  }: { parts: Part[]; currency?: string; width?: string; height?: number; /** total the strip is scaled to; null = the parts' own sum */ scale?: number | null; legend?: boolean; blur?: boolean } = $props();
  const shown = $derived(parts.filter((p) => Math.abs(p.value) >= 0.005));
  const total = $derived(scale ?? (shown.reduce((s, p) => s + Math.abs(p.value), 0) || 1));
  const alpha = (i: number) => Math.max(0.35, 1 - i * 0.18);
  const title = $derived(shown.map((p) => `${p.label} ${formatSigned(p.value, currency)}`).join(" · "));
</script>

<span class="strip" style="width:{width}" {title}>
  <span class="track" style="height:{height}px">
    {#each shown as p, i (p.label)}
      <i class:up={p.value > 0} class:down={p.value < 0} style="width:{(Math.abs(p.value) / total) * 100}%;opacity:{alpha(i)}"></i>
    {/each}
  </span>
  {#if legend}
    <span class="legend">
      {#each shown as p, i (p.label)}
        <span><i class:up={p.value > 0} class:down={p.value < 0} style="opacity:{alpha(i)}"></i>{p.label} <b class="money">{blur ? "•••" : formatSigned(p.value, currency)}</b></span>
      {/each}
    </span>
  {/if}
</span>

<style>
  .strip { display: inline-flex; flex-direction: column; gap: 8px; vertical-align: middle; }
  .track { display: flex; gap: 2px; width: 100%; border-radius: 3px; overflow: hidden; background: var(--surface-2); }
  .track i { display: block; height: 100%; min-width: 1px; background: var(--fill); }
  i.up { background: var(--positive); }
  i.down { background: var(--negative); }
  .legend { display: flex; gap: 14px; flex-wrap: wrap; font-size: 12px; color: var(--text-2); }
  .legend i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; background: var(--fill); }
  .legend b { font-weight: 500; color: var(--text); }
</style>
