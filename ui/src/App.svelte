<script lang="ts">
  import { onMount } from "svelte";
  import { view } from "./lib/stores/view.svelte";
  import { route } from "./lib/stores/route.svelte";
  import { prefs } from "./lib/stores/prefs.svelte";
  import Rail from "./lib/components/shell/Rail.svelte";
  import ProblemBanner from "./lib/components/shell/ProblemBanner.svelte";
  import Home from "./screens/home/Home.svelte";
  import CheckIn from "./screens/home/CheckIn.svelte";
  import AccountDetail from "./screens/home/AccountDetail.svelte";
  import Portfolio from "./screens/portfolio/Portfolio.svelte";
  import Changes from "./screens/portfolio/Changes.svelte";
  import Plan from "./screens/Plan.svelte";
  import Wall from "./screens/home/Wall.svelte";
  import Backups from "./screens/settings/Backups.svelte";
  import Budget from "./screens/budget/Budget.svelte";
  import Play from "./screens/budget/Play.svelte";
  import Settings from "./screens/settings/Settings.svelte";
  import Palette from "./lib/components/shell/Palette.svelte";
  import ChatDrawer from "./lib/components/chat/ChatDrawer.svelte";
  import { drawer } from "./lib/stores/drawer.svelte";

  let palette = $state(false);

  // <svelte:body class:…> is not applied in Svelte 5, so the privacy blur sets the class itself.
  $effect(() => { document.body.classList.toggle("blur", prefs.blur); });

  onMount(() => {
    void view.load();
    view.connect();
    return () => view.disconnect();
  });

  // #/chat (links, the palette, old bookmarks) opens the drawer over the screen you came from.
  let lastScreen = "#/";
  $effect(() => {
    if (route.current.name === "chat") {
      drawer.show();
      location.replace(lastScreen);
    } else {
      lastScreen = location.hash || "#/";
    }
  });

  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "h") {
      e.preventDefault();
      prefs.toggleBlur();
    } else if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "j") {
      e.preventDefault();
      drawer.toggle();
    } else if (e.key === "Escape" && drawer.open && !palette) {
      drawer.hide();
    } else if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      palette = !palette;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<Palette view={view.current} bind:open={palette} />
{#if route.current.name === "wall" && view.current}
  <Wall view={view.current} />
{:else}
<div class="app">
  <Rail onPalette={() => (palette = true)} />
  <main class:docked={drawer.open}>
    <div class="col" class:wide={route.current.name === "changes"}>
      {#if view.current?.problem}
        <ProblemBanner problem={view.current.problem} path={view.current.ledger_path} />
      {/if}
      {#if view.error && !view.current}
        <p class="error">Could not reach the server: {view.error}</p>
      {:else if !view.current}
        <p class="faint">Loading…</p>
      {:else if route.current.name === "checkin"}
        <CheckIn view={view.current} />
      {:else if route.current.name === "portfolio"}
        <Portfolio view={view.current} tab={route.current.tab} />
      {:else if route.current.name === "changes"}
        <Changes view={view.current} />
      {:else if route.current.name === "plan"}
        <Plan view={view.current} />
      {:else if route.current.name === "budget"}
        <Budget view={view.current} month={route.current.month} importing={route.current.import ?? false} />
      {:else if route.current.name === "play"}
        <Play view={view.current} month={route.current.month} />
      {:else if route.current.name === "settings"}
        <Settings view={view.current} />
      {:else if route.current.name === "backups"}
        <Backups view={view.current} />
      {:else if route.current.name === "account"}
        <AccountDetail view={view.current} id={route.current.id} />
      {:else}
        <Home view={view.current} />
      {/if}
    </div>
  </main>
</div>
{#if drawer.open && view.current}<ChatDrawer view={view.current} />{/if}
{/if}

<style>
  .app { display: flex; min-height: 100vh; }
  main { flex: 1; min-width: 0; padding: 28px 28px 48px; }
  main.docked { margin-right: 420px; }
  .col { max-width: 960px; margin: 0 auto; display: flex; flex-direction: column; gap: 20px; }
  .col.wide { max-width: 1360px; }
  .error { color: var(--negative); }
  @media (max-width: 760px) { .app { flex-direction: column; } main { padding: 16px 16px 84px; } main.docked { margin-right: 0; } }
</style>
