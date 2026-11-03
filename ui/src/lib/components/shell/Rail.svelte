<script lang="ts">
  import { route } from "../../stores/route.svelte";
  import { view } from "../../stores/view.svelte";
  import { drawer } from "../../stores/drawer.svelte";
  import { tick } from "svelte";
  import { changes } from "../../stores/portfolio/changes.svelte";
  import { formatDate, formatMoney, formatSigned } from "../../format/money";
  let { onPalette }: { onPalette?: () => void } = $props();
  const waiting = $derived(view.current?.pending.length ?? 0);
  const name = $derived(route.current.name);
  // The rail's live reading: net worth and its last move, on every screen but Home (whose hero shows it).
  const v = $derived(view.current);
  const prev = $derived(v?.deltas.previous ?? null);

  // One brass bookmark that slides to the current screen, instead of a rule that blinks between items.
  let rail: HTMLElement;
  let mark = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  function place() {
    const on = rail?.querySelector<HTMLElement>(".nav[aria-current='page']");
    mark = on && on.offsetParent ? { x: on.offsetLeft, y: on.offsetTop, w: on.offsetWidth, h: on.offsetHeight } : null;
  }
  // Re-measure after the screen changes and after the reading above the items appears.
  $effect(() => { void name; void v?.accounts.length; void tick().then(place); });
  $effect(() => {
    const ro = new ResizeObserver(place);
    ro.observe(rail);
    return () => ro.disconnect();
  });
</script>

<nav class="rail" aria-label="Main" bind:this={rail}>
  {#if mark}<span class="mark" aria-hidden="true" style="--x:{mark.x}px;--y:{mark.y}px;--w:{mark.w}px;--h:{mark.h}px"></span>{/if}
  <a class="brand" href="#/"><svg class="logo" viewBox="0 0 72 72" width="20" height="20" aria-hidden="true"><rect x="4" y="4" width="64" height="64" rx="14" /><path class="base" d="M16 50 H56" /><path class="line" d="M16 44 L27 36 L35 40 L45 26 L56 20" /></svg>Worthwatch</a>
  {#if v && v.accounts.length && name !== "home"}
    <a class="reading" href="#/changes" onclick={() => { if (prev && v) changes.setCustom(prev.from, v.as_of); }} title="What moved it: Changes">
      <span class="nw money">{formatMoney(v.totals.net_worth, v.base_currency)}</span>
      {#if prev}<span class="d" class:down={Number(prev.amount) < 0}><span class="money">{formatSigned(prev.amount, v.base_currency)}</span> since {formatDate(prev.from)}</span>{/if}
    </a>
  {/if}
  <a class="nav" class:on={name === "home" || name === "account"} aria-current={name === "home" || name === "account" ? "page" : undefined} href="#/">Home</a>
  <a class="nav" class:on={name === "checkin"} aria-current={name === "checkin" ? "page" : undefined} href="#/check-in">Check-in</a>
  <a class="nav" class:on={name === "portfolio"} aria-current={name === "portfolio" ? "page" : undefined} href="#/portfolio">Portfolio</a>
  <a class="nav" class:on={name === "changes"} aria-current={name === "changes" ? "page" : undefined} href="#/changes">Changes</a>
  <a class="nav wide" class:on={name === "plan"} aria-current={name === "plan" ? "page" : undefined} href="#/plan">Plan</a>
  <a class="nav wide" class:on={name === "budget" || name === "play"} aria-current={name === "budget" || name === "play" ? "page" : undefined} href="#/budget">Budget</a>
  <button type="button" class="nav wide ask" class:on={drawer.open} aria-expanded={drawer.open} onclick={() => drawer.toggle()} title="Ask a question or type balances and trades (⌘J)">Ask or update{#if waiting}<span class="badge" title="{waiting} proposal{waiting === 1 ? '' : 's'} waiting">{waiting}</span>{:else}<kbd>⌘J</kbd>{/if}</button>
  <button class="nav more" type="button" onclick={() => onPalette?.()} aria-label="More: Plan, Budget, Ask or update, and everything else">More{#if waiting}<span class="badge">{waiting}</span>{/if}</button>
  <div class="spacer"></div>
  <!-- Wall view, Settings, Backups, Hide amounts and Theme live in ⌘K (More). -->
  <button class="foot" type="button" onclick={() => onPalette?.()} title="Settings, Backups, Wall view, Hide amounts (⌘⇧H), Theme, and everything else">More <kbd>⌘K</kbd></button>
</nav>

<style>
  /* Sticky: the reading, the screens and the utilities stay in reach on long pages. */
  .rail { width: 172px; flex: 0 0 auto; position: sticky; top: 0; align-self: flex-start; height: 100vh; overflow-y: auto; border-right: 1px solid var(--border); padding: 20px 0 16px; display: flex; flex-direction: column; gap: 2px; }
  .brand { display: flex; align-items: center; gap: 8px; font-weight: 600; color: var(--text); padding: 6px 20px 10px; font-size: 13px; text-decoration: none; }
  .logo rect { fill: var(--surface); stroke: var(--border); stroke-width: 3; }
  .logo .base { stroke: var(--text-3); stroke-width: 3; stroke-linecap: round; }
  .logo .line { fill: none; stroke: var(--accent); stroke-width: 5; stroke-linecap: round; stroke-linejoin: round; }
  .reading { display: flex; flex-direction: column; gap: 1px; padding: 0 20px 18px; text-decoration: none; color: var(--text); }
  .reading .nw { font-size: 15px; }
  .reading .d { font-size: 11px; color: var(--positive); }
  .reading .d.down { color: var(--negative); }
  .reading:hover .nw { color: var(--accent); }
  .nav { display: flex; align-items: center; gap: 8px; padding: 8px 20px; color: var(--text-2); text-decoration: none; background: none; border: 0; font: inherit; text-align: left; cursor: pointer; }
  .nav:hover { color: var(--text); }
  /* The current screen: a brass rule at the edge, like a bookmark. */
  .nav.on { color: var(--text); font-weight: 600; }
  .ask.on { box-shadow: inset 2px 0 0 var(--accent); }
  .mark { position: absolute; left: 0; top: 0; width: 2px; height: var(--h); transform: translateY(var(--y)); background: var(--accent); pointer-events: none; }
  @media (prefers-reduced-motion: no-preference) {
    .mark { transition: transform 280ms var(--ease-out), height 280ms var(--ease-out); }
  }
  .more { display: none; }
  .ask kbd { margin-left: auto; }
  .badge { margin-left: auto; font-size: 11px; background: var(--text); color: var(--bg); border-radius: 9px; padding: 0 6px; font-weight: 600; line-height: 16px; }
  .spacer { flex: 1; }
  .foot { font-size: 12px; color: var(--text-3); padding: 5px 20px; background: none; border: 0; text-align: left; font: inherit; font-size: 12px; cursor: pointer; }
  .foot:hover { color: var(--text); }
  kbd { font-family: var(--mono); font-size: 10px; border: 1px solid var(--border); border-radius: 3px; padding: 0 4px; margin-left: 4px; }
  a:focus-visible, button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  /* Phone: a bottom bar with the four screens you use most, and More for the rest (⌘K palette). */
  @media (max-width: 760px) {
    .rail { position: fixed; top: auto; bottom: 0; left: 0; right: 0; z-index: 40; width: auto; height: auto; flex-direction: row; justify-content: space-around; align-items: stretch; overflow: hidden; padding: 0 4px env(safe-area-inset-bottom); border-right: 0; border-top: 1px solid var(--border); background: var(--bg); }
    .brand, .reading, .spacer, .foot, .wide { display: none; }
    .nav, .more { display: flex; flex: 1; justify-content: center; min-height: 52px; padding: 0 4px; font-size: 13px; }
    .nav.wide { display: none; }
    .ask.on { box-shadow: none; }
    .mark { top: auto; bottom: auto; width: var(--w); height: 2px; transform: translate(var(--x), var(--y)); }
    .more .badge { margin-left: 4px; }
  }
</style>
