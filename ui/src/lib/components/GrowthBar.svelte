<script lang="ts">
  // Where a move came from: what you added vs what the market did, as one bar that
  // grows out from zero. Gains stack right, losses stack left in red, and the tick is
  // where you ended up. Works the same when the market fell.
  import type { Split } from "../growth";
  import { formatDate, formatSigned } from "../money";
  let { split, currency = "CAD", label }: { split: Split; currency?: string; label: string } = $props();

  const net = $derived(split.added + split.grown);
  const parts = $derived([
    { key: "added", v: split.added, cls: "added" },
    { key: "grown", v: split.grown, cls: split.grown < 0 ? "loss" : "grown" },
  ]);
  const pos = $derived(parts.reduce((s, p) => s + Math.max(0, p.v), 0));
  const neg = $derived(parts.reduce((s, p) => s + Math.max(0, -p.v), 0));
  // Zero sits where the losses end, so a year with no losses uses the full width.
  const span = $derived(Math.max(pos + neg, 1));
  const zero = $derived((neg / span) * 100);
  const w = (v: number) => (Math.abs(v) / span) * 100;
  const rightOff = (i: number) => parts.slice(0, i).reduce((s, p) => s + (p.v > 0 ? w(p.v) : 0), 0);
  const leftOff = (i: number) => parts.slice(0, i).reduce((s, p) => s + (p.v < 0 ? w(p.v) : 0), 0);
  const tick = $derived(zero + (net / span) * 100);
  const youShare = $derived(split.added > 0 && split.grown > 0 ? Math.round((split.added / net) * 100) : null);
</script>

<section class="gb" aria-label="Where the change came from">
  <div class="h"><span class="label">{label} <span class="money">{formatSigned(net, currency)}</span></span><span class="faint">since {formatDate(split.from, "long")} · same range as the chart</span></div>
  <div class="bar" title="You added {formatSigned(split.added, currency)} · market and other {formatSigned(split.grown, currency)}">
    {#if neg > 0}<span class="zero" style="left:{zero}%"></span>{/if}
    {#each parts as p, i (p.key)}
      {#if Math.abs(p.v) >= 0.5}
        <i class={p.cls} style={p.v > 0 ? `left:${zero + rightOff(i)}%;width:${w(p.v)}%` : `right:${100 - zero + leftOff(i)}%;width:${w(p.v)}%`}></i>
      {/if}
    {/each}
    <span class="tick" style="left:{tick}%"></span>
  </div>
  <div class="key">
    <span><i class="added"></i>You added <b class="money">{formatSigned(split.added, currency)}</b></span>
    <span><i class={split.grown < 0 ? "loss" : "grown"}></i>Market and other <b class="money">{formatSigned(split.grown, currency)}</b></span>
    {#if youShare != null}<span class="share">{youShare}% from you, {100 - youShare}% from the market</span>{/if}
  </div>
</section>

<style>
  .gb { border-top: 1px solid var(--border); padding-top: 14px; }
  .h { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; font-size: 12px; }
  .bar { position: relative; height: 14px; margin: 12px 0 10px; display: flex; }
  .bar i { border-radius: 1px; }
  .bar i { position: absolute; top: 0; bottom: 0; }
  .zero { position: absolute; top: -4px; bottom: -4px; width: 1px; background: var(--text-3); }
  .tick { position: absolute; top: -5px; bottom: -5px; width: 2px; margin-left: -2px; background: var(--text); }
  .added { background: var(--g-cash); }
  .grown { background: var(--accent); }
  .loss { background: var(--negative); }
  .key { display: flex; flex-wrap: wrap; gap: 6px 18px; font-size: 12px; color: var(--text-2); }
  .key i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 6px; }
  .key b { color: var(--text); font-weight: 500; }
  .share { margin-left: auto; }
</style>
