<script lang="ts">
  import { onMount } from "svelte";
  import { view } from "./lib/stores/view.svelte";
  import { route } from "./lib/stores/route.svelte";
  import { prefs } from "./lib/stores/prefs.svelte";
  import Rail from "./lib/components/Rail.svelte";
  import ProblemBanner from "./lib/components/ProblemBanner.svelte";
  import Home from "./screens/Home.svelte";
  import CheckIn from "./screens/CheckIn.svelte";
  import AccountDetail from "./screens/AccountDetail.svelte";
  import Portfolio from "./screens/Portfolio.svelte";
  import Changes from "./screens/Changes.svelte";
  import Chat from "./screens/Chat.svelte";
  import Plan from "./screens/Plan.svelte";
  import Wall from "./screens/Wall.svelte";
  import Backups from "./screens/Backups.svelte";
  import Budget from "./screens/Budget.svelte";
  import Settings from "./screens/Settings.svelte";
  import Palette from "./lib/components/Palette.svelte";

  let palette = $state(false);

  onMount(() => {
    void view.load();
    view.connect();
    return () => view.disconnect();
  });

  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "h") {
      e.preventDefault();
      prefs.toggleBlur();
    } else if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      palette = !palette;
    }
  }
</script>

<svelte:window onkeydown={onKey} />
<svelte:body class:blur={prefs.blur} />

<Palette view={view.current} bind:open={palette} />
{#if route.current.name === "wall" && view.current}
  <Wall view={view.current} />
{:else}
<div class="app">
  <Rail onPalette={() => (palette = true)} />
  <main class:tight={route.current.name === "chat"}>
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
      {:else if route.current.name === "chat"}
        <Chat view={view.current} />
      {:else if route.current.name === "plan"}
        <Plan view={view.current} />
      {:else if route.current.name === "budget"}
        <Budget view={view.current} month={route.current.month} importing={route.current.import ?? false} />
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
{/if}

<style>
  .app { display: flex; min-height: 100vh; }
  main { flex: 1; min-width: 0; padding: 28px 28px 48px; }
  main.tight { padding-bottom: 16px; }
  .col { max-width: 820px; margin: 0 auto; display: flex; flex-direction: column; gap: 20px; }
  .col.wide { max-width: 1360px; }
  .error { color: var(--negative); }
  @media (max-width: 760px) { .app { flex-direction: column; } main { padding: 16px 16px 40px; } }
</style>
