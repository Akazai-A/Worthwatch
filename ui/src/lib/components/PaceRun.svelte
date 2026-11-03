<script lang="ts">
  // DRAFT for the gamification exploration: days on pace this month, and months kept.
  import type { BudgetView } from "../api";
  import { monthLabel, monthsKept, paceDays } from "../budget";
  import { formatMoney } from "../money";
  import { prefs } from "../stores/prefs.svelte";

  let { v }: { v: BudgetView } = $props();
  const p = $derived(paceDays(v));
  const months = $derived(monthsKept(v));
  const past = $derived(months.filter((m) => !m.current));
  const kept = $derived(past.filter((m) => m.kept).length);
  const banked = $derived(past.reduce((a, m) => a + (m.budget - m.spent), 0));
  const money = (n: number) => (prefs.blur ? "*****" : formatMoney(n, v.base_currency, 0));
  const mon = $derived(monthLabel(v.month).split(" ")[0].slice(0, 3));
  const mmax = $derived(Math.max(1, ...months.map((m) => m.spent / Math.max(1, m.budget))) );
  const short = (m: string) => monthLabel(m + "-01").slice(0, 3);
  function tip(d: (typeof p.days)[number]): string {
    if (d.state === "future") return `${mon} ${d.day}`;
    const diff = d.pace - d.total;
    return `${mon} ${d.day} · ${money(d.spent)} that day · ${money(d.total)} so far, ${money(Math.abs(diff))} ${diff >= 0 ? "under" : "over"} pace${d.noSpend ? " · no-spend day" : ""}`;
  }
</script>

<section class="card run">
  <div class="hrow"><h2 class="label">Days on pace</h2><span class="faint small">a day counts when the running total ends it at or under pace · hover a day</span></div>
  <div class="grid">
    <div class="score">
      {#if p.run > 0}
        <div class="n">{p.run}</div>
        <div class="sub">day{p.run === 1 ? "" : "s"} in a row<br /><span class="faint">best {p.best} · {p.under} of {p.days.filter((d) => d.state !== "future").length} days</span></div>
      {:else}
        <div class="n neg">0</div>
        <div class="sub">over pace since {mon} {p.overSince}<br /><span class="faint">best {p.best} this month</span></div>
      {/if}
    </div>
    <div class="strip">
      <div class="cells" style="grid-template-columns: repeat({v.days}, 1fr)">
        {#each p.days as d}
          <span class="cell {d.state}" title={tip(d)}>{#if d.noSpend && d.state !== "future"}<i></i>{/if}</span>
        {/each}
      </div>
      <div class="axis" style="grid-template-columns: repeat({v.days}, 1fr)">
        {#each p.days as d}<span>{d.day === 1 || d.day % 7 === 1 ? (d.day === 1 ? `${mon} 1` : d.day) : ""}</span>{/each}
      </div>
    </div>
    <div class="months">
      <div class="bars">
        {#each months as m}
          {@const h = (m.spent / Math.max(1, m.budget) / mmax) * 100}
          <div class="mcol" title="{monthLabel(m.month + '-01')}: {money(m.spent)} of {money(m.budget)}{m.current ? ' pace so far' : ''} · {m.kept ? 'kept' : 'over'}">
            <div class="track"><span class="line" style="bottom:{(1 / mmax) * 100}%"></span><i class:over={!m.kept} class:cur={m.current} style="height:{h}%"></i></div>
            <span class="ml" class:cur={m.current}>{short(m.month)}</span>
          </div>
        {/each}
      </div>
      <div class="sub nw">kept <b>{kept} of {past.length}</b> months<br /><span class:pos={banked >= 0} class:neg={banked < 0} class="money">{money(Math.abs(banked))}</span> <span class="faint">{banked >= 0 ? "under" : "over"} in total</span><br /><a href="#/plan" class="small">see it on Plan ›</a></div>
    </div>
  </div>
  <div class="legend">
    <span><i class="sw under"></i>At or under pace</span>
    <span><i class="sw over"></i>Over pace</span>
    <span><i class="sw under dot"><i></i></i>No-spend day</span>
    <span><i class="sw today"></i>Today</span>
    <span><i class="sw mo"></i>Month vs budget (line is 100%)</span>
  </div>
</section>

<style>
  .hrow { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .card h2 { margin: 0 0 14px; font-weight: 500; }
  .small { font-size: 12px; }
  .grid { display: grid; grid-template-columns: 150px 1fr auto; gap: 28px; align-items: center; }
  .score { display: flex; gap: 12px; align-items: center; }
  .n { font-family: var(--display); font-size: 3rem; line-height: 1; font-weight: 500; font-variant-numeric: lining-nums tabular-nums; color: var(--positive); }
  .sub { font-size: 12px; color: var(--text-2); line-height: 1.5; }
  .nw { white-space: nowrap; }
  .sub b { font-weight: 500; color: var(--text); }
  .cells, .axis { display: grid; gap: 3px; }
  .cell { height: 26px; border-radius: 3px; background: var(--surface-2); position: relative; display: flex; align-items: center; justify-content: center; }
  .cell.under, .cell.today-under { background: color-mix(in oklab, var(--positive) 72%, var(--surface-2)); }
  .cell.over, .cell.today-over { background: color-mix(in oklab, var(--negative) 78%, var(--surface-2)); }
  .cell.today-under, .cell.today-over { box-shadow: 0 0 0 2px var(--bg), 0 0 0 3.5px var(--accent); }
  .cell.future { background: transparent; box-shadow: inset 0 0 0 1px var(--border); }
  .cell i { width: 7px; height: 7px; border-radius: 50%; border: 1.5px solid var(--bg); display: block; }
  .axis span { font-size: 10px; color: var(--text-3); white-space: nowrap; overflow: visible; margin-top: 5px; font-family: var(--mono); }
  .months { display: flex; gap: 14px; align-items: center; }
  .bars { display: flex; gap: 5px; align-items: flex-end; }
  .mcol { display: flex; flex-direction: column; align-items: center; gap: 4px; }
  .track { width: 12px; height: 40px; position: relative; display: flex; align-items: flex-end; }
  .track i { display: block; width: 100%; border-radius: 2px 2px 0 0; background: color-mix(in oklab, var(--positive) 72%, var(--surface-2)); }
  .track i.over { background: color-mix(in oklab, var(--negative) 78%, var(--surface-2)); }
  .track i.cur { opacity: 0.55; }
  .track .line { position: absolute; left: -2px; right: -2px; height: 0; border-top: 1px dashed var(--text-3); }
  .ml { font-size: 10px; color: var(--text-3); font-family: var(--mono); }
  .ml.cur { color: var(--accent); }
  .pos { color: var(--positive); }
  .neg { color: var(--negative) !important; }
  .legend { display: flex; gap: 16px; font-size: 11px; color: var(--text-2); margin-top: 12px; flex-wrap: wrap; align-items: center; }
  .legend span { display: flex; align-items: center; gap: 6px; }
  .sw { width: 12px; height: 12px; border-radius: 3px; display: inline-flex; align-items: center; justify-content: center; }
  .sw.under { background: color-mix(in oklab, var(--positive) 72%, var(--surface-2)); }
  .sw.over { background: color-mix(in oklab, var(--negative) 78%, var(--surface-2)); }
  .sw.dot i { width: 5px; height: 5px; border-radius: 50%; border: 1.5px solid var(--bg); }
  .sw.today { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .sw.mo { width: 8px; background: color-mix(in oklab, var(--positive) 72%, var(--surface-2)); border-radius: 2px 2px 0 0; }
  @media (max-width: 760px) {
    .grid { grid-template-columns: 1fr; gap: 14px; }
    .cell { height: 18px; }
  }
</style>
