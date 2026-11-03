<script lang="ts">
  import { route } from "../stores/route.svelte";
  import { prefs } from "../stores/prefs.svelte";
  import { view } from "../stores/view.svelte";
  const waiting = $derived(view.current?.pending.length ?? 0);
  const name = $derived(route.current.name);
  const themeLabel = $derived(prefs.theme === "system" ? "Auto" : prefs.theme === "dark" ? "Dark" : "Light");
</script>

<nav class="rail" aria-label="Main">
  <a class="brand" href="#/">worthwatch</a>
  <a class="nav" class:on={name === "home" || name === "account"} href="#/"><span class="dot"></span>Home</a>
  <a class="nav" class:on={name === "checkin"} href="#/check-in"><span class="dot"></span>Check-in</a>
  <a class="nav" class:on={name === "portfolio"} href="#/portfolio"><span class="dot"></span>Portfolio</a>
  <a class="nav" class:on={name === "changes"} href="#/changes"><span class="dot"></span>Changes</a>
  <a class="nav" class:on={name === "chat"} href="#/chat"><span class="dot"></span>Chat{#if waiting}<span class="badge" title="{waiting} proposal{waiting === 1 ? '' : 's'} waiting">{waiting}</span>{/if}</a>
  <div class="spacer"></div>
  <button class="foot" type="button" onclick={() => prefs.toggleBlur()} title="Blur amounts (⌘⇧H)">
    {prefs.blur ? "Unblur" : "Blur"} amounts <kbd>⌘⇧H</kbd>
  </button>
  <button class="foot" type="button" onclick={() => prefs.cycleTheme()}>Theme: {themeLabel}</button>
</nav>

<style>
  .rail { width: 180px; flex: 0 0 auto; border-right: 1px solid var(--border); padding: 20px 12px; display: flex; flex-direction: column; gap: 4px; }
  .brand { font-weight: 600; letter-spacing: 0.04em; color: var(--text-2); padding: 6px 10px 18px; font-size: 13px; text-decoration: none; }
  .nav { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 8px; color: var(--text-2); text-decoration: none; }
  a.nav:hover { background: var(--surface); }
  .nav.on { background: var(--accent-soft); color: var(--text); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--text-3); }
  .nav.on .dot { background: var(--accent); }
  .badge { margin-left: auto; font-size: 10px; background: var(--text); color: var(--bg); border-radius: 9px; padding: 0 6px; font-weight: 600; line-height: 16px; }
  .spacer { flex: 1; }
  .foot { font-size: 11px; color: var(--text-3); padding: 6px 10px; background: none; border: 0; text-align: left; font: inherit; font-size: 11px; cursor: pointer; border-radius: 6px; }
  .foot:hover { color: var(--text-2); background: var(--surface); }
  kbd { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; border: 1px solid var(--border); border-radius: 4px; padding: 0 4px; margin-left: 4px; }
  a:focus-visible, button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (max-width: 760px) {
    .rail { width: auto; flex-direction: row; border-right: 0; border-bottom: 1px solid var(--border); padding: 8px 12px; overflow-x: auto; align-items: center; }
    .brand { padding: 6px 10px; }
    .nav { white-space: nowrap; }
    .spacer, kbd { display: none; }
  }
</style>
