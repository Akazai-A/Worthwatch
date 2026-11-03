<script lang="ts">
  // One account, month by month: a column per month split into what you added (blue)
  // and what grew (brass), losses below the line in red. Drawn at the width it is given.
  import type { MonthBar } from "../networth/growth";
  import { formatCompact, formatSigned } from "../format/money";
  let { months, currency = "CAD", blur = false }: { months: MonthBar[]; currency?: string; blur?: boolean } = $props();
  let cw = $state(800);
  const H = 120, TOP = 18, FOOT = 20;
  const up = (m: MonthBar) => Math.max(0, m.added) + Math.max(0, m.grown);
  const dn = (m: MonthBar) => Math.max(0, -m.added) + Math.max(0, -m.grown);
  const pmax = $derived(Math.max(0, ...months.map(up)));
  const nmax = $derived(Math.max(0, ...months.map(dn)));
  const scale = $derived(H / Math.max(1, pmax + nmax));
  const zero = $derived(TOP + pmax * scale);
  const slot = $derived(cw / Math.max(1, months.length));
  const bw = $derived(Math.min(56, slot * 0.7));
  const label = (m: string) => new Date(m + "-01T00:00:00Z").toLocaleString("en-CA", { month: "short", timeZone: "UTC" });
  const sig = (v: number) => (blur ? "*****" : formatSigned(v, currency));
  const short = (v: number) => (blur ? "" : `${v > 0 ? "+" : v < 0 ? "−" : ""}${formatCompact(Math.abs(v))}`);
  let hover = $state<number | null>(null);
</script>

<div class="mb" bind:clientWidth={cw}>
  <svg width={cw} height={TOP + H + FOOT} role="img" aria-label="Month by month: what you added and what grew">
    <line class="z" x1="0" x2={cw} y1={zero} y2={zero} />
    {#each months as m, i (m.month)}
      {@const x = i * slot + (slot - bw) / 2}
      {@const a = Math.max(0, m.added) * scale}
      {@const g = Math.max(0, m.grown) * scale}
      {@const an = Math.max(0, -m.added) * scale}
      {@const gn = Math.max(0, -m.grown) * scale}
      <g onmouseenter={() => (hover = i)} onmouseleave={() => (hover = null)} role="presentation">
        <rect class="hit" x={i * slot} y="0" width={slot} height={TOP + H} />
        {#if a}<rect class="added" x={x} y={zero - a} width={bw} height={a} />{/if}
        {#if g}<rect class="grown" x={x} y={zero - a - g} width={bw} height={g} />{/if}
        {#if an}<rect class="added neg" x={x} y={zero} width={bw} height={an} />{/if}
        {#if gn}<rect class="loss" x={x} y={zero + an} width={bw} height={gn} />{/if}
        {#if m.added || m.grown}<text class="v" x={x + bw / 2} y={zero - a - g - 5} text-anchor="middle">{short(m.added + m.grown)}</text>{/if}
        <text class="m" x={x + bw / 2} y={TOP + H + 15} text-anchor="middle">{label(m.month)}</text>
      </g>
    {/each}
  </svg>
  {#if hover != null}
    {@const m = months[hover]}
    <div class="tip" style="left:{Math.min(cw - 200, Math.max(0, hover * slot))}px">
      <b>{label(m.month)} {m.month.slice(0, 4)}</b>
      <span><i class="added"></i>added <b class="money">{sig(m.added)}</b></span>
      <span><i class={m.grown < 0 ? "loss" : "grown"}></i>{m.grown < 0 ? "lost" : "grew"} <b class="money">{sig(m.grown)}</b></span>
    </div>
  {/if}
</div>

<style>
  .mb { position: relative; }
  svg { display: block; overflow: visible; }
  .z { stroke: var(--border); }
  .hit { fill: transparent; }
  .added { fill: var(--g-cash); }
  .added.neg { opacity: 0.55; }
  .grown { fill: var(--accent); }
  .loss { fill: var(--negative); }
  .v { font-family: var(--figures); font-size: 10px; fill: var(--text-2); }
  .m { font-size: 11px; fill: var(--text-3); }
  .tip { position: absolute; top: 0; width: 190px; background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px; font-size: 12px; display: flex; flex-direction: column; gap: 2px; pointer-events: none; }
  .tip i { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 6px; }
  .tip i.added { background: var(--g-cash); } .tip i.grown { background: var(--accent); } .tip i.loss { background: var(--negative); }
</style>
