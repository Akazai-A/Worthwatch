<script lang="ts">
  import { formatMoney } from "../money";
  let { value, currency = "CAD", size = "m" }: { value: string | number; currency?: string; size?: "s" | "m" | "xl" } = $props();
</script>

<span class="money num {size}">{formatMoney(value, currency)}</span>

<style>
  .money { font-variant-numeric: tabular-nums; }
  .s { font-size: 0.875rem; color: var(--text-2); }
  .xl { font-size: 3.5rem; font-weight: 600; letter-spacing: -0.02em; }
</style>
