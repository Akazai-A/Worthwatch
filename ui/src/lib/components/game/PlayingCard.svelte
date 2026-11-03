<script lang="ts">
  // A challenge as a playing card. The art is the progress: the cup fills as you spend
  // toward the cap; the squares tick off clean days. Offers carry Play and Pass.
  import type { GameCard } from "../../api";
  import { cup, glyph, squares, SUITS } from "../../budget/game";
  import Glyph from "./Glyph.svelte";

  let { c, i = 0, money, onPlay, onPass }: {
    c: GameCard; i?: number; money: (n: number, digits?: number) => string;
    onPlay?: () => void; onPass?: () => void;
  } = $props();
  const suit = $derived(SUITS[i % SUITS.length]);
  const fill = $derived(cup(c));
  const sq = $derived(squares(c));
  const meta = $derived.by(() => {
    if (c.status === "won") return Number(c.saved) > 0 ? `${money(Number(c.saved))} under your usual` : "won";
    if (c.status === "lost") return "lost this week";
    if (c.rule.kind === "cap") {
      const left = Number(c.rule.amount) - Number(c.spent);
      return c.status === "offer" ? `${c.from ? "today to Sunday · " : ""}usually ${money(Number(c.usual))}` : `${money(left, 2)} left this week`;
    }
    return c.status === "offer" ? (c.from ? "today to Sunday" : "this week") : `${c.days_done} of ${c.days_total} days`;
  });
</script>

<article class="card {c.status}" aria-label="{c.title}: {c.status}">
  <span class="corner">{suit}</span><span class="corner br">{suit}</span>
  <div class="art">
    {#if c.art === "cup" && c.category !== "coffee"}
      <svg viewBox="0 0 90 90" width="92" height="92" aria-hidden="true">
        <defs><clipPath id="tube-{c.id}"><rect x="30" y="14" width="30" height="66" rx="12" /></clipPath></defs>
        <rect x="30" y="14" width="30" height="66" rx="12" class="mug" />
        <rect x="0" y={80 - 66 * fill.fill} width="90" height={66 * fill.fill} clip-path="url(#tube-{c.id})" class="coffee" class:spilled={fill.spilled} />
        <path d="M24 14 H66" class="handle" />
        <text x="72" y="18" class="capl">cap</text>
      </svg>
    {:else if c.art === "cup"}
      <svg viewBox="0 0 90 90" width="92" height="92" aria-hidden="true">
        <defs><clipPath id="cup-{c.id}"><path d="M18 28 H64 L60 70 Q58 78 50 78 H32 Q24 78 22 70 Z" /></clipPath></defs>
        <path d="M18 28 H64 L60 70 Q58 78 50 78 H32 Q24 78 22 70 Z" class="mug" />
        <rect x="0" y={78 - 50 * fill.fill} width="90" height={50 * fill.fill} clip-path="url(#cup-{c.id})" class="coffee" class:spilled={fill.spilled} />
        <path d="M64 36 Q80 36 78 50 Q76 62 62 60" class="handle" />
        {#if c.status !== "won"}<path d="M34 20 q2 -6 0 -10 M42 20 q2 -6 0 -10 M50 20 q2 -6 0 -10" class="steam" />{/if}
        {#if fill.spilled}<path d="M60 74 q8 4 14 2 q-4 6 -12 4" class="spill" />{/if}
      </svg>
    {:else}
      <div class="sq" style="--n:{Math.min(5, sq.length)}">
        {#each sq as s}<span class={s}></span>{/each}
      </div>
    {/if}
    <span class="badge"><Glyph name={glyph(c.category)} size={16} /></span>
  </div>
  <h3>{c.title}</h3>
  <p class="meta">{meta}</p>
  {#if c.status === "offer"}
    <div class="acts">
      <button type="button" class="play" onclick={onPlay}>Play</button>
      <button type="button" class="pass" onclick={onPass}>Pass</button>
    </div>
  {:else if c.status === "won"}
    <p class="stamp">Won</p>
  {:else if c.status === "lost"}
    <p class="stamp">✕</p>
  {/if}
</article>

<style>
  .card {
    width: 168px; height: 226px; flex: none; border-radius: 14px; position: relative; display: flex; flex-direction: column; align-items: center;
    padding: 16px 12px 12px; text-align: center; background: linear-gradient(170deg, var(--surface-2), var(--surface));
    border: 1px solid var(--border); box-shadow: 0 10px 24px rgba(0, 0, 0, 0.22); transition: transform 0.2s ease, box-shadow 0.2s ease;
  }
  .card:hover { transform: translateY(-4px) rotate(-0.6deg); box-shadow: 0 14px 30px rgba(0, 0, 0, 0.28); }
  .card.playing { border-color: var(--accent); box-shadow: 0 0 0 1px color-mix(in oklab, var(--accent) 50%, transparent), 0 12px 28px rgba(0, 0, 0, 0.26); }
  .card.won { background: var(--gold-card); border-color: var(--accent); color: var(--on-gold); }
  .card.lost { opacity: 0.55; filter: grayscale(0.6); }
  .card.offer { border-style: dashed; }
  .corner { position: absolute; top: 9px; left: 11px; font-family: var(--display); color: var(--accent); font-size: 14px; line-height: 1; }
  .won .corner { color: var(--on-gold); }
  .corner.br { top: auto; left: auto; bottom: 9px; right: 11px; transform: rotate(180deg); }
  .art { flex: 1; display: flex; align-items: center; justify-content: center; position: relative; width: 100%; }
  .badge { position: absolute; right: 2px; top: 0; color: var(--text-3); }
  .won .badge { color: var(--on-gold); }
  .mug { fill: var(--surface-2); stroke: var(--text-3); stroke-width: 2; }
  .won .mug { fill: none; stroke: var(--on-gold); }
  .coffee { fill: var(--accent); opacity: 0.85; transition: y 0.6s ease, height 0.6s ease; }
  .coffee.spilled, .spill { fill: var(--negative); }
  .won .coffee { fill: var(--on-gold); opacity: 0.35; }
  .handle, .steam { fill: none; stroke: var(--text-3); stroke-width: 2; }
  .won .handle { stroke: var(--on-gold); }
  .steam { stroke-width: 1.5; opacity: 0.6; }
  .capl { font-family: var(--mono); font-size: 8px; fill: var(--text-3); }
  .won .capl { fill: var(--on-gold); }
  .sq { display: grid; grid-template-columns: repeat(var(--n), 16px); gap: 8px; justify-content: center; }
  .sq span { width: 16px; height: 16px; border-radius: 4px; border: 2px solid var(--text-3); }
  .sq .done { background: var(--positive); border-color: var(--positive); }
  .won .sq .done { background: var(--on-gold); border-color: var(--on-gold); }
  .sq .broken { background: var(--negative); border-color: var(--negative); }
  h3 { font-family: var(--display); font-weight: 500; font-size: 17px; line-height: 1.15; margin: 0; }
  .meta { font-size: 11px; color: var(--text-3); margin: 4px 0 0; min-height: 15px; }
  .won .meta { color: var(--on-gold); opacity: 0.8; }
  .acts { display: flex; gap: 6px; margin-top: 8px; }
  .acts button { font: inherit; font-size: 12px; border-radius: 999px; padding: 3px 12px; cursor: pointer; }
  .play { background: var(--accent); border: 1px solid var(--accent); color: var(--bg); font-weight: 600 !important; }
  .pass { background: none; border: 1px solid var(--border); color: var(--text-2); }
  .stamp { margin: 8px 0 0; font-family: var(--display); font-size: 14px; letter-spacing: 0.08em; text-transform: uppercase; }
  .lost .stamp { color: var(--negative); }
  @media (prefers-reduced-motion: reduce) { .card:hover { transform: none; } }
</style>
