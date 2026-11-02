<script lang="ts">
  // Companies sized by your dollars, grouped into sector blocks. Each block carries one
  // muted family hue (tech blue, financials rose, consumer sage, industry plum; bonds and
  // the rest neutral); within a block the bigger a cell, the stronger its tint. Brass marks
  // only the selection. Squarified layout from d3-hierarchy; cells are buttons so the keyboard
  // reaches them. Hover shows a tooltip, click selects (the parent shows the detail).
  import { hierarchy, treemap, treemapSquarify, type HierarchyRectangularNode } from "d3-hierarchy";
  import type { Company } from "../api";
  import { formatMoney } from "../money";
  import { FAMILY_ORDER, familyHue, familyOf, viaSummary } from "../lookthrough";

  let {
    companies,
    currency,
    selected = null,
    height = 380,
    onselect,
  }: { companies: Company[]; currency: string; selected?: string | null; height?: number; onselect?: (key: string) => void } = $props();

  let width = $state(0);
  type Cell = { c: Company; x: number; y: number; w: number; h: number; tone: number; hue: string };
  type Block = { name: string; x: number; y: number; w: number; h: number; pct: number; hue: string };
  type Node = { name?: string; children?: Node[]; c?: Company };
  const HEAD = 18;
  const TAIL = "Smaller names";
  const layout = $derived.by<{ cells: Cell[]; blocks: Block[] }>(() => {
    if (width < 10 || companies.length === 0) return { cells: [], blocks: [] };
    const groups = new Map<string, Company[]>();
    for (const c of companies) {
      const fam = c.key === "__other__" ? TAIL : c.undisclosed ? "Other" : familyOf(c.slot);
      (groups.get(fam) ?? groups.set(fam, []).get(fam)!).push(c);
    }
    const order = new Map(FAMILY_ORDER.map((f, i) => [f, i]));
    const children: Node[] = [...groups.entries()]
      .sort((a, b) => (a[0] === TAIL ? 100 : (order.get(a[0] as never) ?? 99)) - (b[0] === TAIL ? 100 : (order.get(b[0] as never) ?? 99)))
      .map(([name, cs]) => ({ name, children: cs.map((c) => ({ c })) }));
    const root = hierarchy<Node>({ children })
      .sum((d) => (d.c ? Number(d.c.value) : 0))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
    const total = root.value || 1;
    const laid = treemap<Node>()
      .size([width, height])
      .paddingOuter(3)
      .paddingTop(HEAD + 3)
      .paddingInner(2)
      .tile(treemapSquarify.ratio(1.3))(root) as HierarchyRectangularNode<Node>;
    const blocks: Block[] = (laid.children ?? []).map((b) => ({
      name: b.data.name!,
      x: b.x0, y: b.y0, w: b.x1 - b.x0, h: b.y1 - b.y0,
      pct: ((b.value ?? 0) / total) * 100,
      hue: familyHue(b.data.name as never),
    }));
    const cells: Cell[] = [];
    for (const b of laid.children ?? []) {
      const leaves = b.leaves();
      const n = Math.max(leaves.length - 1, 1);
      const hue = familyHue(b.data.name as never);
      leaves.forEach((leaf, i) => {
        // rank 0 = biggest in its block → strongest tint. Smaller cells fade toward the surface.
        cells.push({ c: leaf.data.c!, x: leaf.x0, y: leaf.y0, w: leaf.x1 - leaf.x0, h: leaf.y1 - leaf.y0, tone: 58 - 34 * (i / n), hue });
      });
    }
    return { cells, blocks };
  });

  let tip = $state<{ c: Company; x: number; y: number } | null>(null);
  function move(e: MouseEvent, c: Company) {
    const box = (e.currentTarget as HTMLElement).parentElement!.getBoundingClientRect();
    tip = { c, x: e.clientX - box.left, y: e.clientY - box.top };
  }
  const tipLeft = $derived(tip ? (tip.x > width * 0.6 ? tip.x - 12 : tip.x + 12) : 0);
</script>

<div class="treemap" style="height:{height}px" bind:clientWidth={width} role="group" aria-label="Companies by value, grouped by sector">
  {#each layout.blocks as b (b.name)}
    <div class="block" style="left:{b.x}px;top:{b.y}px;width:{b.w}px;height:{b.h}px;--hue:var({b.hue})" aria-hidden="true">
      {#if b.w > 70}<span class="head"><span class="bn">{b.name}</span>{#if b.w > 150}<span class="num">{b.pct.toFixed(0)}%</span>{/if}</span>{/if}
    </div>
  {/each}
  {#each layout.cells as cell (cell.c.key)}
    {@const big = cell.w > 48 && cell.h > 26}
    <button
      type="button"
      class="cell"
      class:sel={selected === cell.c.key}
      class:undisclosed={cell.c.undisclosed}
      class:aggregate={cell.c.key === "__other__"}
      style="left:{cell.x}px;top:{cell.y}px;width:{cell.w}px;height:{cell.h}px;--tone:{cell.tone.toFixed(1)}%;--hue:var({cell.hue})"
      aria-label="{cell.c.name}, {formatMoney(cell.c.value, currency)}, {Number(cell.c.pct).toFixed(1)} percent"
      onclick={() => onselect?.(cell.c.key)}
      onmousemove={(e) => move(e, cell.c)}
      onmouseleave={() => (tip = null)}
      onfocus={() => (tip = null)}>
      {#if big}
        <b>{cell.c.name}</b>
        {#if cell.h > 40}<span class="num">{Number(cell.c.pct).toFixed(1)}%</span>{/if}
      {/if}
    </button>
  {/each}
  {#if tip}
    <div class="tip" style="left:{tipLeft}px;top:{tip.y + 14}px" class:flip={tip.x > width * 0.6}>
      <b>{tip.c.name}</b>{#if tip.c.ticker && tip.c.ticker !== tip.c.name}&nbsp;<span class="faint">{tip.c.ticker}</span>{/if}
      <div><span class="money">{formatMoney(tip.c.value, currency)}</span> · <span class="num">{Number(tip.c.pct).toFixed(1)}%</span>{#if tip.c.sector}&nbsp;· {tip.c.sector}{/if}</div>
      {#if tip.c.via.length}<div class="faint">{viaSummary(tip.c)}</div>{/if}
    </div>
  {/if}
</div>

<style>
  .treemap { position: relative; width: 100%; overflow: hidden; border-radius: 6px; background: var(--surface-2); }
  .block { position: absolute; border-radius: 4px; background: color-mix(in oklab, var(--hue) 7%, var(--surface-2)); }
  .head { position: absolute; left: 6px; right: 6px; top: 0; height: 18px; display: flex; justify-content: space-between; align-items: center; font-size: 10px; letter-spacing: 0.06em; text-transform: uppercase; color: color-mix(in oklab, var(--hue) 70%, var(--text)); white-space: nowrap; overflow: hidden; }
  .head .bn { overflow: hidden; text-overflow: ellipsis; }
  .cell { position: absolute; overflow: hidden; border: 0; border-radius: 3px; padding: 5px 6px; font: inherit; font-size: 11px; line-height: 1.25; text-align: left; cursor: pointer; display: flex; flex-direction: column; align-items: flex-start; gap: 1px; color: var(--text); background: color-mix(in oklab, var(--hue) var(--tone), var(--surface-2)); }
  .cell b { display: block; max-width: 100%; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cell span { color: var(--text-2); }
  .cell:hover { background: color-mix(in oklab, var(--hue) calc(var(--tone) + 10%), var(--surface-2)); z-index: 1; }
  .cell.sel { outline: 2px solid var(--accent); outline-offset: -2px; z-index: 2; }
  .cell.sel b { color: var(--accent); }
  .cell:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; z-index: 2; }
  .cell.undisclosed, .cell.aggregate { background-image: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in oklab, var(--text) 10%, transparent) 6px 8px); }
  .tip { position: absolute; z-index: 5; pointer-events: none; background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 6px 9px; font-size: 12px; color: var(--text-2); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25); max-width: 260px; white-space: nowrap; }
  .tip.flip { transform: translateX(-100%); }
  .tip b { color: var(--text); font-weight: 600; }
  .tip .money { color: var(--text); }
</style>
