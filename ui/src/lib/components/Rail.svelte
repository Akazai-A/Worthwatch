<script lang="ts">
  import { route } from "../stores/route.svelte";
  import { prefs } from "../stores/prefs.svelte";
  import { view } from "../stores/view.svelte";
  import { formatDate, formatMoney, formatSigned } from "../money";
  let { onPalette }: { onPalette?: () => void } = $props();
  const waiting = $derived(view.current?.pending.length ?? 0);
  const name = $derived(route.current.name);
  const themeLabel = $derived(prefs.theme === "system" ? "Auto" : prefs.theme === "dark" ? "Dark" : "Light");
  // The rail's live reading: net worth and its last move, visible from every screen.
  const v = $derived(view.current);
  const prev = $derived(v?.deltas.previous ?? null);
</script>

<nav class="rail" aria-label="Main">
  <a class="brand" href="#/">Worthwatch</a>
  {#if v && v.accounts.length}
    <a class="reading" href="#/changes" title="What moved it: Changes">
      <span class="nw money">{formatMoney(v.totals.net_worth, v.base_currency)}</span>
      {#if prev}<span class="d" class:down={Number(prev.amount) < 0}><span class="money">{formatSigned(prev.amount, v.base_currency)}</span> since {formatDate(prev.from)}</span>{/if}
    </a>
  {/if}
  <a class="nav" class:on={name === "home" || name === "account"} aria-current={name === "home" || name === "account" ? "page" : undefined} href="#/">Home</a>
  <a class="nav" class:on={name === "checkin"} aria-current={name === "checkin" ? "page" : undefined} href="#/check-in">Check-in</a>
  <a class="nav" class:on={name === "portfolio"} aria-current={name === "portfolio" ? "page" : undefined} href="#/portfolio">Portfolio</a>
  <a class="nav" class:on={name === "changes"} aria-current={name === "changes" ? "page" : undefined} href="#/changes">Changes</a>
  <a class="nav wide" class:on={name === "plan"} aria-current={name === "plan" ? "page" : undefined} href="#/plan">Plan</a>
  <a class="nav wide" class:on={name === "budget"} aria-current={name === "budget" ? "page" : undefined} href="#/budget">Budget</a>
  <a class="nav wide" class:on={name === "chat"} aria-current={name === "chat" ? "page" : undefined} href="#/chat">Chat{#if waiting}<span class="badge" title="{waiting} proposal{waiting === 1 ? '' : 's'} waiting">{waiting}</span>{/if}</a>
  <button class="nav more" type="button" onclick={() => onPalette?.()} aria-label="More: Plan, Budget, Chat and everything else">More{#if waiting}<span class="badge">{waiting}</span>{/if}</button>
  <div class="spacer"></div>
  <button class="foot" type="button" onclick={() => onPalette?.()} title="Jump anywhere (⌘K)">Go to… <kbd>⌘K</kbd></button>
  <a class="foot" href="#/wall">Wall view</a>
  <a class="foot" class:cur={name === "settings"} href="#/settings">Settings</a>
  <a class="foot" class:cur={name === "backups"} href="#/backups">Backups</a>
  <button class="foot" type="button" onclick={() => prefs.toggleBlur()} title="Hide amounts (⌘⇧H)">
    {prefs.blur ? "Show" : "Hide"} amounts <kbd>⌘⇧H</kbd>
  </button>
  <button class="foot" type="button" onclick={() => prefs.cycleTheme()}>Theme: {themeLabel}</button>
</nav>

<style>
  /* Sticky: the reading, the screens and the utilities stay in reach on long pages. */
  .rail { width: 172px; flex: 0 0 auto; position: sticky; top: 0; align-self: flex-start; height: 100vh; overflow-y: auto; border-right: 1px solid var(--border); padding: 20px 0 16px; display: flex; flex-direction: column; gap: 2px; }
  .brand { font-weight: 600; color: var(--text-2); padding: 6px 20px 10px; font-size: 13px; text-decoration: none; }
  .reading { display: flex; flex-direction: column; gap: 1px; padding: 0 20px 18px; text-decoration: none; color: var(--text); }
  .reading .nw { font-size: 15px; }
  .reading .d { font-size: 11px; color: var(--positive); }
  .reading .d.down { color: var(--negative); }
  .reading:hover .nw { color: var(--accent); }
  .nav { display: flex; align-items: center; gap: 8px; padding: 8px 20px; color: var(--text-2); text-decoration: none; background: none; border: 0; font: inherit; text-align: left; cursor: pointer; }
  .nav:hover { color: var(--text); }
  /* The current screen: a brass rule at the edge, like a bookmark. */
  .nav.on { color: var(--text); font-weight: 600; box-shadow: inset 2px 0 0 var(--accent); }
  .more { display: none; }
  .badge { margin-left: auto; font-size: 11px; background: var(--text); color: var(--bg); border-radius: 9px; padding: 0 6px; font-weight: 600; line-height: 16px; }
  .spacer { flex: 1; }
  .foot { font-size: 12px; color: var(--text-3); padding: 5px 20px; background: none; border: 0; text-align: left; font: inherit; font-size: 12px; cursor: pointer; }
  a.foot { text-decoration: none; }
  .foot.cur, .foot:hover { color: var(--text); }
  kbd { font-family: var(--mono); font-size: 10px; border: 1px solid var(--border); border-radius: 3px; padding: 0 4px; margin-left: 4px; }
  a:focus-visible, button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  /* Phone: a bottom bar with the four screens you use most, and More for the rest (⌘K palette). */
  @media (max-width: 760px) {
    .rail { position: fixed; top: auto; bottom: 0; left: 0; right: 0; z-index: 40; width: auto; height: auto; flex-direction: row; justify-content: space-around; align-items: stretch; overflow: hidden; padding: 0 4px env(safe-area-inset-bottom); border-right: 0; border-top: 1px solid var(--border); background: var(--bg); }
    .brand, .reading, .spacer, .foot, .wide { display: none; }
    .nav, .more { display: flex; flex: 1; justify-content: center; min-height: 52px; padding: 0 4px; font-size: 13px; }
    .nav.on { box-shadow: inset 0 2px 0 var(--accent); }
    .more .badge { margin-left: 4px; }
  }
</style>
