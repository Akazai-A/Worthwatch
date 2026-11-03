<script lang="ts">
  // Small line icons for card categories. One stroke colour, no fills, so they sit on
  // plain and gold cards alike.
  let { name, size = 18 }: { name: "cup" | "bowl" | "cart" | "bag" | "ticket" | "bus" | "sun" | "coin"; size?: number } = $props();
</script>

<svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
  {#if name === "cup"}<path d="M5 9h11l-1 9a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2zM16 11h2a2 2 0 0 1 0 4h-2M8 3v3M11 3v3" />
  {:else if name === "bowl"}<path d="M3 11h18a9 9 0 0 1-18 0zM8 7l2-4M13 7l3-4" />
  {:else if name === "cart"}<path d="M3 4h3l2 11h10l2-8H7M9 20h.01M17 20h.01" />
  {:else if name === "bag"}<path d="M5 8h14l-1 12H6zM9 8V6a3 3 0 0 1 6 0v2" />
  {:else if name === "ticket"}<path d="M3 7h18v3a2 2 0 0 0 0 4v3H3v-3a2 2 0 0 0 0-4zM14 7v10" />
  {:else if name === "bus"}<path d="M5 4h14v13H5zM5 11h14M8 20v-3M16 20v-3" />
  {:else if name === "sun"}<path d="M12 7a5 5 0 1 1 0 10 5 5 0 0 1 0-10zM12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M5 19l1.5-1.5M17.5 6.5L19 5" />
  {:else}<path d="M12 4a8 8 0 1 1 0 16 8 8 0 0 1 0-16zM12 8v8M10 10h3a1.5 1.5 0 0 1 0 3h-2a1.5 1.5 0 0 0 0 3h3" />{/if}
</svg>

<style>
  svg { display: block; }
  path { fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
</style>
