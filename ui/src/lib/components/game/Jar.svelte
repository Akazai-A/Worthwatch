<script lang="ts">
  // The jar: coins up to how much the closed days left over, a dashed line at the goal.
  import { jarCoins } from "../../game";
  import CoinDefs from "./CoinDefs.svelte";

  let { fraction, goalLabel = null, size = 1, id = "coin-jar", glow = false }: {
    fraction: number; goalLabel?: string | null; size?: number; id?: string; glow?: boolean;
  } = $props();
  // The goal sits at 80% of the glass, so a jar past its goal visibly overflows the line.
  const TOP = 40, BOTTOM = 186, L = 42, R = 138;
  const goalY = BOTTOM - 0.8 * (BOTTOM - TOP);
  const coins = $derived(jarCoins(fraction * 0.8, L, R, BOTTOM, BOTTOM - TOP));
</script>

<svg viewBox="20 6 150 190" width={150 * size} height={190 * size} role="img" aria-label="Jar {Math.round(fraction * 100)}% of the goal">
  <CoinDefs {id} />
  <defs>
    <clipPath id="{id}-glass"><path d="M40 34 H140 V168 Q140 188 120 188 H60 Q40 188 40 168 Z" /></clipPath>
  </defs>
  {#if glow}<circle cx="90" cy="110" r="78" class="glow" />{/if}
  <path d="M40 34 H140 V168 Q140 188 120 188 H60 Q40 188 40 168 Z" class="glass" />
  <rect x="34" y="20" width="112" height="15" rx="4" class="lid" />
  <g clip-path="url(#{id}-glass)">
    {#each coins as c, i}<ellipse cx={c.cx} cy={c.cy} rx="14" ry="5" fill="url(#{id})" class="coin" style="--i:{i}" />{/each}
  </g>
  {#if goalLabel}
    <line x1="34" y1={goalY} x2="146" y2={goalY} class="goal" />
    <text x="150" y={goalY + 3} class="gl">{goalLabel}</text>
  {/if}
  <path d="M52 44 V160" class="shine" />
</svg>

<style>
  svg { display: block; overflow: visible; }
  .glass { fill: color-mix(in oklab, var(--surface-2) 70%, transparent); stroke: var(--border); stroke-width: 2; }
  .lid { fill: var(--fill); }
  .coin { stroke: var(--coin-lo); stroke-width: 0.6; animation: fall 0.45s ease-out both; animation-delay: calc(var(--i) * 12ms); }
  .goal { stroke: var(--accent); stroke-width: 1.5; stroke-dasharray: 4 3; }
  .gl { font-family: var(--mono); font-size: 9px; fill: var(--accent); }
  .shine { stroke: var(--text); stroke-opacity: 0.07; stroke-width: 6; stroke-linecap: round; }
  .glow { fill: var(--accent); opacity: 0.12; filter: blur(12px); }
  @keyframes fall { from { transform: translateY(-40px); opacity: 0; } to { transform: none; opacity: 1; } }
</style>
