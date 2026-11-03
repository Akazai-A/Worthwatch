<script lang="ts">
  // Portfolio › Overlap: fund × fund shared weight. Click a cell for the shared names.
  import type { LookThroughView } from "../lib/api";
  import { formatMoney } from "../lib/money";
  import { compactMoney } from "../lib/lookthrough";
  import Money from "../lib/components/Money.svelte";

  let { lt }: { lt: LookThroughView } = $props();
  const cur = $derived(lt.base_currency);
  const o = $derived(lt.overlap);
  // Only funds with a file make a row; a stock or a fund without one has nothing to share.
  const idx = $derived(o.symbols.map((_, i) => i).filter((i) => o.cells[i][i] != null));
  const leftOut = $derived(o.symbols.length - idx.length);
  const n = $derived(idx.length);
  const compact = $derived(n > 8);
  let pick = $state<[number, number] | null>(null);
  // Default to the highest off-diagonal pair.
  const best = $derived.by<[number, number] | null>(() => {
    let b: [number, number] | null = null;
    let top = -1;
    for (const i of idx) for (const j of idx) {
      if (j <= i) continue;
      const c = o.cells[i][j];
      if (c && Number(c.pct) > top) { top = Number(c.pct); b = [i, j]; }
    }
    return b;
  });
  const sel = $derived(pick ?? best);
  const cell = $derived(sel ? o.cells[sel[0]][sel[1]] : null);
  const resolvedCount = $derived(n);
  function shade(pct: string): string {
    const a = Math.round(6 + 44 * (Number(pct) / 100));
    return `color-mix(in oklab, var(--text) ${a}%, var(--surface-2))`;
  }
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
    <section class="card grid">
      <table class="matrix" class:compact aria-label="Overlap between funds">
        <thead><tr><th></th>{#each idx as j (j)}<th>{o.symbols[j]}</th>{/each}</tr></thead>
        <tbody>
          {#each idx as i (i)}
            {@const a = o.symbols[i]}
            <tr>
              <th class="rh">{a}</th>
              {#each idx as j (j)}
                {@const b = o.symbols[j]}
                {@const c = o.cells[i][j]}
                {#if !c}
                  <td class="none"><span>—</span></td>
                {:else if i === j}
                  <td class="diag" style="background:{shade('100')}" title="{a}: effective number of holdings {o.effective_holdings[i] ?? '—'}"><span class="num">100%</span>{#if !compact}<small>~{o.effective_holdings[i] ?? "—"} eff.</small>{/if}</td>
                {:else}
                  <td>
                    <button type="button" class:on={sel?.[0] === i && sel?.[1] === j} style="background:{shade(c.pct)}" onclick={() => (pick = [i, j])} title="{a} × {b}: {compactMoney(c.shared_value)} shared" aria-label="{a} and {b}: {c.pct} percent shared, {formatMoney(c.shared_value, cur)}">
                      <span class="num">{Number(c.pct).toFixed(0)}%</span>{#if !compact}<small class="money">{compactMoney(c.shared_value)} shared</small>{/if}
                    </button>
                  </td>
                {/if}
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="faint note">Shared weight = Σ min(weight in A, weight in B) over every underlying name, through funds of funds. The diagonal shows each fund's effective number of holdings.{#if leftOut}&nbsp;{leftOut} {leftOut === 1 ? "holding" : "holdings"} without a file {leftOut === 1 ? "is" : "are"} left out.{/if}</p>
    </section>

    <section class="card detail" aria-live="polite">
      {#if sel && cell}
        <div class="label">{o.symbols[sel[0]]} × {o.symbols[sel[1]]}</div>
        <h3>{Number(cell.pct).toFixed(0)}% the same</h3>
        <div class="sub2"><Money value={cell.shared_value} currency={cur} /> of your {o.symbols[sel[0]]} sits in names {o.symbols[sel[1]]} also holds</div>
        <table class="shared">
          <thead><tr><th>Shared name</th><th class="r">{o.symbols[sel[0]]}</th><th class="r">{o.symbols[sel[1]]}</th></tr></thead>
          <tbody>
            {#each cell.shared as s (s.name)}
              <tr><td>{s.name}</td><td class="r num">{Number(s.a_pct).toFixed(1)}%</td><td class="r num">{Number(s.b_pct).toFixed(1)}%</td></tr>
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
  .two { display: grid; grid-template-columns: minmax(0, max-content) minmax(300px, 1fr); gap: 20px; align-items: start; }
  .grid { padding: 12px 14px; overflow-x: auto; max-width: 100%; min-width: 0; }
  .grid .note { max-width: 44ch; }
  .matrix { border-collapse: separate; border-spacing: 3px; font-size: 12px; }
  .matrix th { text-align: center; padding: 4px; font-size: 11px; font-weight: 500; color: var(--text-2); }
  .matrix th.rh { text-align: right; padding-right: 8px; white-space: nowrap; }
  .matrix td { padding: 0; width: 92px; height: 52px; border-radius: 4px; text-align: center; vertical-align: middle; color: var(--text); }
  .matrix td button { width: 100%; height: 52px; border: 0; border-radius: 4px; font: inherit; color: var(--text); cursor: pointer; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 1px; }
  .matrix td button:hover { filter: brightness(1.1); }
  .matrix td button.on { outline: 2px solid var(--accent); }
  .matrix td button:focus-visible { outline: 2px solid var(--accent); }
  .matrix td.diag, .matrix td.none { display: table-cell; }
  .matrix td.diag { padding: 0 6px; }
  .matrix td.none { background: var(--surface-2); color: var(--text-3); }
  .matrix small { display: block; font-size: 10px; color: var(--text-2); font-family: var(--font); }
  .matrix.compact td, .matrix.compact td button { width: 44px; height: 32px; font-size: 11px; }
  .matrix.compact th { font-size: 10px; padding: 2px; }
  .matrix.compact th.rh { padding-right: 6px; }
  .detail h3 { margin: 2px 0 0; font-size: 20px; font-weight: 600; }
  .sub2 { font-size: 12px; color: var(--text-3); margin-top: 2px; }
  .shared { width: 100%; border-collapse: collapse; font-size: 13px; margin-top: 12px; }
  .shared th { text-align: left; font-weight: 500; color: var(--text-3); font-size: 12px; padding: 4px 0; }
  .shared td { padding: 6px 0; border-top: 1px solid var(--border); }
  .r { text-align: right; }
  .note { font-size: 12px; margin: 12px 0 0; }
  @media (max-width: 760px) { .two { grid-template-columns: 1fr; } }
</style>
