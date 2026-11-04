<script lang="ts">
  import { formatMoney, formatSigned } from "../../format/money";
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
  // Headline figures roll like a counter when the value changes (a new check-in, a price, a scrubbed day).
  // Columns are keyed from the right, so the ones digit stays the ones digit as the figure grows.
  const cols = $derived([...text].reverse().map((c, i) => ({ c, key: i, d: /\d/.test(c) ? Number(c) : null })).reverse());
</script>

{#if size === "xl"}
  <span
    class="money xl"
    class:neg={tone === "auto" && n < 0}
    class:down={tone === "delta" && n < 0}
    class:up={tone === "delta" && n > 0}><span class="sr">{text}</span>{#each cols as col (col.key)}{#if col.d == null}<span class="ch" aria-hidden="true" data-c={col.c}></span>{:else}<span class="dg" aria-hidden="true"><span class="reel" style="--d:{col.d}"></span></span>{/if}{/each}</span>
{:else}
<span
  class="money {size}"
  class:neg={tone === "auto" && n < 0}
  class:down={tone === "delta" && n < 0}
  class:up={tone === "delta" && n > 0}>{text}</span>
{/if}

<style>
  .s { font-size: 0.875rem; }
  .l { font-size: 1.25rem; font-weight: 500; }
  /* Headline figures: the serif display face, lining tabular numerals, no tight tracking (it read "$279 , 877"). */
  .xl { font-family: var(--display); font-size: 4.25rem; font-weight: 500; letter-spacing: -0.01em; line-height: 1; font-variant-numeric: lining-nums tabular-nums; }
  .xl { display: inline-flex; white-space: nowrap; }
  /* One reel of 0–9 per digit, clipped to a single line and moved to the current digit. The glyphs are
     generated content, so the element's text (copy, tests, screen readers) is just the figure in .sr. */
  /* Rows sit --row apart, taller than a glyph, so the next digit never peeks in under the window. */
  .dg { --row: 1.4em; display: inline-block; height: 1.12em; margin-bottom: -0.12em; overflow: hidden; contain: paint; line-height: var(--row); }
  .reel { display: block; transform: translateY(calc(var(--d) * -1 * var(--row) - (var(--row) - 1em) / 2)); }
  .reel::before { content: "0\A 1\A 2\A 3\A 4\A 5\A 6\A 7\A 8\A 9"; white-space: pre; display: block; }
  .ch::before { content: attr(data-c); }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  @media (prefers-reduced-motion: no-preference) {
    .reel { transition: transform 420ms var(--ease-out); }
  }
  .neg, .down { color: var(--negative); }
  .up { color: var(--positive); }
  @media (max-width: 760px) { .xl { font-size: 3.25rem; } }
</style>
