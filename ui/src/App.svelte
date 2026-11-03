<script lang="ts">
  import { onMount } from "svelte";
  import { view } from "./lib/stores/view.svelte";
  import { route } from "./lib/stores/route.svelte";
  import { prefs } from "./lib/stores/prefs.svelte";
  import Rail from "./lib/components/shell/Rail.svelte";
  import ProblemBanner from "./lib/components/shell/ProblemBanner.svelte";
  import type { Component } from "svelte";
  import Home from "./screens/home/Home.svelte";
  import Palette from "./lib/components/shell/Palette.svelte";
  import ChatDrawer from "./lib/components/chat/ChatDrawer.svelte";
  import UndoBar from "./lib/components/shell/UndoBar.svelte";
  import { drawer } from "./lib/stores/drawer.svelte";

  let palette = $state(false);

  // Home ships in the first load; every other screen loads its code the first time you open it.
  // The screen you are leaving stays up until the next one is ready, so nothing flashes.
  type Name = Exclude<typeof route.current.name, "home" | "chat">;
  const loaders: Record<Name, () => Promise<{ default: Component<any> }>> = {
    checkin: () => import("./screens/home/CheckIn.svelte"),
    account: () => import("./screens/home/AccountDetail.svelte"),
    portfolio: () => import("./screens/portfolio/Portfolio.svelte"),
    changes: () => import("./screens/portfolio/Changes.svelte"),
    plan: () => import("./screens/Plan.svelte"),
    wall: () => import("./screens/home/Wall.svelte"),
    backups: () => import("./screens/settings/Backups.svelte"),
    budget: () => import("./screens/budget/Budget.svelte"),
    play: () => import("./screens/budget/Play.svelte"),
    settings: () => import("./screens/settings/Settings.svelte"),
  };
  const loaded = $state<Partial<Record<Name, Component<any>>>>({});
  const wanted = $derived<Name | null>(route.current.name === "home" || route.current.name === "chat" ? null : route.current.name);
  let shown = $state<Name | "home">("home");
  $effect(() => {
    const n = wanted;
    if (n == null) { shown = "home"; return; }
    if (loaded[n]) { shown = n; return; }
    void loaders[n]().then((m) => { loaded[n] = m.default; if (wanted === n) shown = n; });
  });
  const Screen = $derived(shown === "home" ? null : loaded[shown] ?? null);

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
{#if shown === "wall" && Screen && view.current}
  <Screen view={view.current} />
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
      {:else if Screen && shown !== "home" && shown !== "wall"}
        {@const r = route.current}
        <Screen view={view.current} tab={r.name === "portfolio" ? r.tab : undefined} month={r.name === "budget" || r.name === "play" ? r.month : undefined} importing={r.name === "budget" ? (r.import ?? false) : undefined} id={r.name === "account" ? r.id : undefined} />
      {:else}
        <Home view={view.current} />
      {/if}
    </div>
  </main>
</div>
{#if drawer.open && view.current}<ChatDrawer view={view.current} />{/if}
<UndoBar />
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
