<script lang="ts">
  import { hashFor, type PortfolioTab } from "../../stores/route.svelte";
  let { tab }: { tab: PortfolioTab } = $props();
  const TABS: { tab: PortfolioTab; label: string }[] = [
    { tab: "positions", label: "Positions" },
    { tab: "lookthrough", label: "Look-through" },
    { tab: "exposure", label: "Exposure" },
    { tab: "overlap", label: "Overlap" },
  ];
</script>

<nav class="tabs" aria-label="Portfolio sections">
  {#each TABS as t (t.tab)}
    <a href={hashFor({ name: "portfolio", tab: t.tab })} class:on={tab === t.tab} aria-current={tab === t.tab ? "page" : undefined}>{t.label}</a>
  {/each}
</nav>

<style>
  .tabs { display: flex; gap: 2px; border-bottom: 1px solid var(--border); }
  a { padding: 8px 12px; color: var(--text-2); font-size: 13px; text-decoration: none; border-bottom: 2px solid transparent; margin-bottom: -1px; }
  a:hover { color: var(--text); }
  a.on { color: var(--text); border-bottom-color: var(--accent); }
  a:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; border-radius: 4px; }
</style>
