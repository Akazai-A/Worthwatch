<script lang="ts">
  // Portfolio › Overlap: every pair of funds once, ranked by how much they hold in
  // common. Click a pair for the shared names.
  import type { LookThroughView } from "../../lib/api";
  import { compactMoney } from "../../lib/portfolio/lookthrough";
  import Money from "../../lib/components/amounts/Money.svelte";
  import { formatPct } from "../../lib/format/money";

  let { lt }: { lt: LookThroughView } = $props();
  const cur = $derived(lt.base_currency);
  const o = $derived(lt.overlap);
  // Only funds with a file make a row; a stock or a fund without one has nothing to share.
  const idx = $derived(o.symbols.map((_, i) => i).filter((i) => o.cells[i][i] != null));
  const leftOut = $derived(o.symbols.length - idx.length);
  const n = $derived(idx.length);
  // Each pair once, biggest overlap first; the side with fewer dollars in common is named
  // first so the sentence reads "$X of A is already in B".
  const pairs = $derived.by(() => {
    const out: { i: number; j: number; pct: number; value: number }[] = [];
    for (const a of idx) for (const b of idx) {
      if (b <= a) continue;
      const ab = o.cells[a][b], ba = o.cells[b][a];
      if (!ab || !ba) continue;
      const [i, j] = Number(ab.shared_value) <= Number(ba.shared_value) ? [a, b] : [b, a];
      out.push({ i, j, pct: Number(ab.pct), value: Number(o.cells[i][j]!.shared_value) });
    }
    return out.sort((x, y) => y.pct - x.pct);
  });
  let showAll = $state(false);
  const shown = $derived(showAll ? pairs : pairs.slice(0, 8));
  let pick = $state<[number, number] | null>(null);
  // Default to the highest pair.
  const best = $derived<[number, number] | null>(pairs[0] ? [pairs[0].i, pairs[0].j] : null);
  const sel = $derived(pick ?? best);
  const cell = $derived(sel ? o.cells[sel[0]][sel[1]] : null);
  const resolvedCount = $derived(n);
</script>

<div class="top">
  <div class="label">Overlap</div>
  <div class="muted lead">How much any two of your funds hold in common, by weight</div>
</div>

{#if resolvedCount < 2}
  <section class="card empty">
    <p class="muted">Overlap needs holdings files for at least two funds you hold{#if resolvedCount === 1}; one is on file{/if}. See the <a href="#/portfolio/look-through">Look-through</a> tab for how to add them.</p>
  </section>
{:else}
  <div class="two">
    <section class="pairs" aria-label="Overlap between funds">
      {#each shown as q (`${q.i}-${q.j}`)}
        {@const on = sel != null && ((sel[0] === q.i && sel[1] === q.j) || (sel[0] === q.j && sel[1] === q.i))}
        <button type="button" class="pair" class:on aria-pressed={on} onclick={() => (pick = [q.i, q.j])}>
          <span class="ph"><span><b>{o.symbols[q.i]}</b> and <b>{o.symbols[q.j]}</b></span><span class="num">{q.pct.toFixed(0)}% the same</span></span>
          <span class="pb"><i class:hi={q.pct >= 80} style="width:{q.pct}%"></i></span>
          <span class="faint ps"><span class="money">{compactMoney(q.value)}</span> of your {o.symbols[q.i]} is already in {o.symbols[q.j]}</span>
        </button>
      {/each}
      {#if pairs.length > shown.length || showAll}
        <button type="button" class="more" onclick={() => (showAll = !showAll)}>{showAll ? "Show fewer" : `Show all ${pairs.length} pairs`}</button>
      {/if}
      <p class="faint note">Shared weight = Σ min(weight in A, weight in B) over every underlying name, through funds of funds.{#if leftOut}&nbsp;{leftOut} {leftOut === 1 ? "holding" : "holdings"} without a file {leftOut === 1 ? "is" : "are"} left out.{/if}</p>
    </section>

    <section class="card detail" aria-live="polite">
      {#if sel && cell}
        <div class="label">{o.symbols[sel[0]]} × {o.symbols[sel[1]]}</div>
        <h3>{formatPct(cell.pct, 0)} the same</h3>
        <div class="sub2"><Money value={cell.shared_value} currency={cur} /> of your {o.symbols[sel[0]]} sits in names {o.symbols[sel[1]]} also holds</div>
        <table class="shared">
          <thead><tr><th>Shared name</th><th class="r">{o.symbols[sel[0]]}</th><th class="r">{o.symbols[sel[1]]}</th></tr></thead>
          <tbody>
            {#each cell.shared as s (s.name)}
              <tr><td>{s.name}</td><td class="r num">{formatPct(s.a_pct)}</td><td class="r num">{formatPct(s.b_pct)}</td></tr>
            {/each}
          </tbody>
        </table>
        {#if lt.lookthrough.effective_holdings}
          <p class="faint note">Your whole portfolio behaves like ~{lt.lookthrough.effective_holdings} equally weighted names.</p>
        {/if}
      {/if}
    </section>
  </div>
{/if}

<style>
  .lead { font-size: 13px; margin-top: 2px; }
  .empty p { margin: 0; }
  .two { display: grid; grid-template-columns: minmax(0, 1fr) minmax(300px, 1fr); gap: 32px; align-items: start; }
  .pairs { min-width: 0; display: flex; flex-direction: column; }
  .pair { display: flex; flex-direction: column; gap: 6px; text-align: left; background: none; border: 0; border-top: 1px solid var(--border); padding: 12px 0 12px 10px; font: inherit; color: var(--text); cursor: pointer; }
  .pair.on { box-shadow: inset 2px 0 0 var(--accent); }
  .pair:hover .ph b { color: var(--accent); }
  .pair:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .ph { display: flex; justify-content: space-between; gap: 12px; }
  .ph b { font-weight: 600; }
  .pb { display: block; height: 10px; background: var(--surface-2); border-radius: 3px; overflow: hidden; }
  .pb i { display: block; height: 100%; background: var(--fill); }
  .pb i.hi { background: var(--warn); }
  .ps { font-size: 12px; }
  .more { align-self: flex-start; background: none; border: 0; padding: 8px 0; font: inherit; font-size: 12px; color: var(--text-2); text-decoration: underline; text-underline-offset: 3px; cursor: pointer; }
  .detail h3 { margin: 2px 0 0; font-size: 20px; font-weight: 600; }
  .sub2 { font-size: 12px; color: var(--text-3); margin-top: 2px; }
  .shared { width: 100%; border-collapse: collapse; font-size: 13px; margin-top: 12px; }
  .shared th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 12px; padding: 4px 0; }
  .shared td { padding: 6px 0; border-top: 1px solid var(--border); }
  .r { text-align: right; }
  .note { font-size: 12px; margin: 12px 0 0; }
  @media (max-width: 760px) { .two { grid-template-columns: 1fr; } }
</style>
