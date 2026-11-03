<script lang="ts">
  // The month as a track: one tile per day, coins on the days that put money in the jar,
  // your token on today with its allowance draining as you tap.
  import type { GameView } from "../../api";
  import { pile, tileTip, track } from "../../game";
  import CoinDefs from "./CoinDefs.svelte";

  let { g, money, mon }: { g: GameView; money: (n: number) => string; mon: string } = $props();
  let width = $state(760);
  // Three rows on a wide screen with the month's end on the last one; five a row on a phone.
  const cols = $derived(width < 560 ? 5 : Math.ceil((g.days + 1) / 3));
  const rowH = $derived(cols === 5 ? 76 : 92);
  // Leave room for the half-circle turns at each row end.
  const t = $derived(track(g.days, cols, width, rowH, rowH / 2 + 16));
  const coin = "coin-track";
  const R = $derived(cols === 5 ? 13 : 16);
  // Today's ring: what's left in green from the top, what's spent in grey after it.
  const ring = $derived.by(() => {
    if (!g.today) return null;
    const a = Number(g.today.allowance) || 1;
    const left = Math.max(0, Number(g.today.left)) / a;
    const r = R + 2;
    return { r, c: 2 * Math.PI * r, left: Math.min(1, left), over: Number(g.today.left) < 0 };
  });
  const endDone = $derived(g.day > g.days);
</script>

<div class="track" bind:clientWidth={width}>
  <svg viewBox="0 0 {t.width} {t.height}" width={t.width} height={t.height} role="img" aria-label="{mon}: one tile per day, coins where money went into the jar">
    <CoinDefs id={coin} />
    <path d={t.path} class="road" />
    <path d={t.path} class="lane" />
    {#each g.tiles as tile, i (tile.day)}
      {@const n = t.nodes[i]}
      <g class="tile {tile.state}" transform="translate({n.x} {n.y})" style="--i:{i}">
        <title>{tileTip(tile, money, mon)}</title>
        {#if tile.state === "today" && ring}
          <circle r={R + 12} class="glow" />
          <circle r={R + 6} class="token" />
          <circle r={ring.r} class="ring-track" />
          <circle r={ring.r} class="ring-spent" class:over={ring.over} />
          <circle r={ring.r} class="ring-left" stroke-dasharray="{ring.left * ring.c} {ring.c}" transform="rotate(-90)" />
          <text y="4" class="today-n">{money(Math.max(0, Number(g.today!.left)))}</text>
          <text y={R + 24} class="cap">today · {g.today!.taps.length} tap{g.today!.taps.length === 1 ? "" : "s"}</text>
        {:else if tile.payday && tile.state === "ahead"}
          <circle r={R + 1} fill="url(#{coin})" class="payday" />
          <text y="4.5" class="pay">$</text>
          <text y={R + 16} class="cap">payday</text>
        {:else}
          <circle r={tile.state === "ahead" ? R - 2 : R} class="disc" />
          <text y="4" class="n">{tile.day}</text>
          {#if tile.payday}<text y={R + 14} class="cap">payday</text>{/if}
        {/if}
        {#if tile.coins > 0}
          <g class="coins" transform="translate(0 {-R - 7})">
            {#each pile(tile.coins) as c, k}<ellipse cx={c.dx} cy={c.dy} rx="4.6" ry="4" fill="url(#{coin})" class="coin" style="--k:{k}" />{/each}
          </g>
        {/if}
        {#if tile.bills.length}
          <path class="house" d="M{tile.coins ? -R - 9 : -5} {-R - 4} l5 -5 5 5 v7 h-10z" />
        {/if}
      </g>
    {/each}
    <g transform="translate({t.end.x} {t.end.y})" class="end" class:done={endDone}>
      <circle r={R + 2} class="vault" />
      <text y="5" class="pay v">$</text>
      <text y={R + 18} class="cap">month closes</text>
    </g>
  </svg>
</div>

<style>
  .track { width: 100%; min-width: 0; }
  svg { display: block; width: 100%; height: auto; overflow: visible; }
  svg text { font-family: var(--mono); text-anchor: middle; }
  .road { fill: none; stroke: var(--border); stroke-width: 22; stroke-linecap: round; }
  .lane { fill: none; stroke: var(--surface-2); stroke-width: 15; stroke-linecap: round; }
  .disc { stroke-width: 2; }
  .green .disc { fill: var(--positive); stroke: none; }
  .red .disc { fill: var(--negative); stroke: none; }
  .ahead .disc { fill: var(--bg); stroke: var(--border); }
  .n { font-size: 11px; fill: var(--bg); font-weight: 600; }
  .ahead .n { fill: var(--text-3); font-weight: 400; }
  .cap { font-size: 9px; fill: var(--text-3); }
  .pay { font-size: 13px; font-weight: 700; fill: var(--coin-lo); }
  .glow { fill: var(--accent); opacity: 0.16; animation: breathe 3s ease-in-out infinite; }
  .token { fill: var(--surface); stroke: var(--border); }
  .ring-track { fill: none; stroke: var(--surface-2); stroke-width: 6; }
  .ring-spent { fill: none; stroke: var(--fill); stroke-width: 6; }
  .ring-spent.over { stroke: var(--negative); }
  .ring-left { fill: none; stroke: var(--positive); stroke-width: 6; transition: stroke-dasharray 0.6s ease; }
  .today-n { font-size: 12px; font-weight: 600; fill: var(--text); }
  .coin { stroke: var(--coin-lo); stroke-width: 0.6; animation: drop 0.5s cubic-bezier(0.3, 1.4, 0.6, 1) both; animation-delay: calc(var(--i) * 25ms + var(--k) * 60ms); }
  .house { fill: var(--text-3); }
  .vault { fill: none; stroke: var(--accent); stroke-width: 2; stroke-dasharray: 3 3; }
  .end.done .vault { fill: url(#coin-track); stroke: var(--coin-lo); stroke-dasharray: none; }
  .v { fill: var(--accent); }
  .end.done .v { fill: var(--coin-lo); }
  @keyframes drop { from { transform: translateY(-14px); opacity: 0; } to { transform: none; opacity: 1; } }
  @keyframes breathe { 50% { opacity: 0.3; } }
</style>
