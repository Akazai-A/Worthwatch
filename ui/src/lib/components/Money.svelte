<script lang="ts">
  import { formatMoney, formatSigned } from "../money";
  let {
    value,
    currency = "CAD",
    size = "m",
    signed = false,
    digits = 0,
    tone = "auto",
  }: {
    value: string | number;
    currency?: string;
    size?: "s" | "m" | "l" | "xl";
    signed?: boolean;
    digits?: number;
    /** auto: negatives muted red; none: inherit colour; delta: red/green by sign */
    tone?: "auto" | "none" | "delta";
  } = $props();
  const n = $derived(Number(value));
  const text = $derived(signed ? formatSigned(value, currency) : formatMoney(value, currency, digits));
</script>

<span
  class="money {size}"
  class:num={size !== "xl"}
  class:neg={tone === "auto" && n < 0}
  class:down={tone === "delta" && n < 0}
  class:up={tone === "delta" && n > 0}>{text}</span>

<style>
  .num { font-variant-numeric: tabular-nums; }
  .s { font-size: 0.875rem; }
  .l { font-size: 1.25rem; font-weight: 600; }
  .xl { font-size: 3.5rem; font-weight: 600; letter-spacing: -0.02em; line-height: 1.05; }
  .neg, .down { color: var(--negative); }
  .up { color: var(--positive); }
  @media (max-width: 760px) { .xl { font-size: 2.75rem; } }
</style>
