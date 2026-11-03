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
  class:neg={tone === "auto" && n < 0}
  class:down={tone === "delta" && n < 0}
  class:up={tone === "delta" && n > 0}>{text}</span>

<style>
  .s { font-size: 0.875rem; }
  .l { font-size: 1.25rem; font-weight: 500; }
  /* Headline figures: the serif display face, lining tabular numerals, no tight tracking (it read "$279 , 877"). */
  .xl { font-family: var(--display); font-size: 4.25rem; font-weight: 500; letter-spacing: -0.01em; line-height: 1; font-variant-numeric: lining-nums tabular-nums; }
  .neg, .down { color: var(--negative); }
  .up { color: var(--positive); }
  @media (max-width: 760px) { .xl { font-size: 3.25rem; } }
</style>
