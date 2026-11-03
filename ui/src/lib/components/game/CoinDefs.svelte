<script lang="ts">
  // The coin gradient, once per SVG. `id` keeps two boards on a page from clashing.
  let { id }: { id: string } = $props();
</script>

<defs>
  <radialGradient {id} cx="35%" cy="35%" r="70%">
    <stop offset="0" style="stop-color: var(--coin-hi)" />
    <stop offset="0.6" style="stop-color: var(--accent)" />
    <stop offset="1" style="stop-color: var(--coin-lo)" />
  </radialGradient>
</defs>
