<script lang="ts">
  // Full-screen, read-only view for a spare tablet or a second monitor: the number,
  // the line with the next 12 months dashed, the asset split and three figures to
  // glance at. No rail. Updates live through the same event stream as every screen.
  import { onMount } from "svelte";
  import { GROUPS, GROUP_LABEL, type LedgerView } from "../../lib/api";
  import { formatCompact, formatDate, formatMoney, formatSigned } from "../../lib/format/money";
  import { monthYear, yearsBetween } from "../../lib/networth/plan";
  import { plan } from "../../lib/stores/plan.svelte";
  import { prefs } from "../../lib/stores/prefs.svelte";
  import FanChart from "../../lib/charts/FanChart.svelte";

  let { view }: { view: LedgerView } = $props();
  const cur = $derived(view.base_currency);
  let now = $state(new Date());
  onMount(() => {
    const t = setInterval(() => (now = new Date()), 30_000);
    return () => clearInterval(t);
  });
  $effect(() => { void plan.ensure(view.version); });

  const P = $derived(plan.current ? { ...plan.current.projection, points: plan.current.projection.points.slice(0, 12) } : null);
  const assets = $derived(Number(view.totals.assets));
  const share = (g: string) => (assets > 0 ? (Number(view.totals.by_group[g as keyof typeof view.totals.by_group] ?? 0) / assets) * 100 : 0);
  const history = $derived.by(() => {
    const cut = Date.parse(`${view.as_of}T00:00:00Z`) - 366 * 86400000;
    return view.series.filter((p) => Date.parse(`${p.date}T00:00:00Z`) >= cut);
  });
  const tfsa = $derived(plan.current?.room.find((r) => r.kind === "tfsa") ?? plan.current?.room[0] ?? null);
  const hide = (s: string) => (prefs.blur ? "*****" : s);
  let h = $state(400);
  const clock = $derived(
    `${now.toLocaleDateString("en-CA", { weekday: "short", month: "short", day: "numeric" })} · ${now.toLocaleTimeString("en-CA", { hour: "2-digit", minute: "2-digit", hour12: false })}`,
  );
  const owedPct = $derived(assets > 0 ? Math.min(100, (Number(view.totals.liabilities) / assets) * 100) : 0);
  // A tablet that shows the Wall all day: everything drifts a few pixels each hour so nothing burns in.
  let hour = $state(new Date().getHours());
  $effect(() => { const t = setInterval(() => (hour = new Date().getHours()), 60000); return () => clearInterval(t); });
  const drift = $derived({ x: [0, 4, 6, 3, -2, -5, -3, 2][hour % 8], y: [0, 3, -2, -4, -1, 3, 5, 1][hour % 8] });
</script>

<div class="wall" class:blur={prefs.blur} style="--dx:{drift.x}px;--dy:{drift.y}px">
  <div class="top"><a href="#/" title="Back to the app">Worthwatch</a><span>net worth · {cur} · as of {formatDate(view.as_of)}</span><span class="clock">{clock}</span></div>
  <div>
    <div class="big money">{formatMoney(view.totals.net_worth, cur)}</div>
    <div class="chips">
      {#if view.deltas.previous}<span><b class:down={Number(view.deltas.previous.amount) < 0} class="money">{formatSigned(view.deltas.previous.amount, cur)}</b> since {formatDate(view.deltas.previous.from)}</span>{/if}
      {#if view.deltas.ytd}<span><b class:down={Number(view.deltas.ytd.amount) < 0} class="money">{formatSigned(view.deltas.ytd.amount, cur)}</b> YTD</span>{/if}
      {#if view.deltas.one_year?.pct != null}<span><b class:down={Number(view.deltas.one_year.pct) < 0}>{Number(view.deltas.one_year.pct) > 0 ? "+" : ""}{view.deltas.one_year.pct}%</b> 1Y</span>{/if}
    </div>
  </div>
  <div class="chart" bind:clientHeight={h}>
    {#if P}
      <FanChart {history} points={P.points} start={P.start} startNetWorth={P.start_net_worth} currency={cur} blur={prefs.blur} height={Math.max(160, h)} compact />
    {/if}
  </div>
  <div class="foot">
    <div>
      <!-- What you own on top; what you owe under it, from the left; the brass box is what's left. -->
      <div class="split">{#each GROUPS.filter((g) => g !== "debt") as g}<i style="width:{share(g)}%;background:var(--g-{g})"></i>{/each}</div>
      <div class="owe"><i class="debt" style="width:{owedPct}%"></i><i class="left" style="width:{Math.max(0, 100 - owedPct)}%"></i></div>
      <div class="lg">
        {#each GROUPS.filter((g) => g !== "debt") as g}<span><i style="background:var(--g-{g})"></i>{GROUP_LABEL[g]} {share(g).toFixed(0)}%</span>{/each}
        <span class="dl"><i class="sq debt"></i>Debt {hide(formatCompact(Number(view.totals.liabilities)))}</span>
        <span class="nl"><i class="sq left"></i>Net worth {hide(formatCompact(Number(view.totals.net_worth)))}</span>
      </div>
    </div>
    {#if P}
      <div class="k"><div class="label">In 1 year</div><div class="v money">{hide(formatCompact(Number(P.points[11].net_worth)))}</div><div class="s">at {P.return_pct}%, {hide(formatCompact(Number(P.monthly_saving)))}/mo saved</div></div>
    {/if}
    {#if tfsa}
      <div class="k"><div class="label">{tfsa.label} room left</div><div class="v money">{hide(formatMoney(tfsa.left, cur))}</div><div class="s">CRA {formatDate(tfsa.as_of)}</div></div>
    {/if}
    {#if plan.current?.projection.target}
      {@const pr = plan.current.projection}
      <div class="k"><div class="label">{hide(formatCompact(Number(pr.target)))} target</div>
        {#if pr.target_date}<div class="v">{monthYear(pr.target_date)}</div><div class="s">{yearsBetween(pr.start, pr.target_date)} years at this pace</div>
        {:else}<div class="v faint">later</div><div class="s">beyond the horizon</div>{/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .wall { height: 100vh; padding: 40px 52px 36px; display: grid; grid-template-rows: auto auto 1fr auto; gap: 22px; box-sizing: border-box; }
  .top { display: flex; justify-content: space-between; align-items: baseline; color: var(--text-3); font-size: 14px; gap: 16px; }
  .top a { color: var(--text-3); text-decoration: none; }
  .clock { font-family: var(--figures); color: var(--text-2); font-size: 22px; }
  .big { font-family: var(--font); font-weight: 600; font-size: clamp(56px, 13vw, 176px); letter-spacing: -0.03em; line-height: 1; font-variant-numeric: lining-nums tabular-nums; }
  .chips { display: flex; gap: 28px; font-family: var(--figures); font-size: clamp(16px, 1.8vw, 22px); margin-top: 12px; color: var(--text-2); flex-wrap: wrap; }
  .chips b { font-weight: 400; color: var(--positive); }
  .chips b.down { color: var(--negative); }
  .chart { min-height: 160px; overflow: hidden; }
  .foot { display: grid; grid-template-columns: 2fr 1fr 1fr 1fr; gap: 28px; align-items: end; }
  .wall { transform: translate(var(--dx), var(--dy)); }
  @media (prefers-reduced-motion: no-preference) { .wall { transition: transform 4s ease-in-out; } }
  .split { display: flex; height: 12px; border-radius: 4px; overflow: hidden; }
  .owe { display: flex; gap: 2px; height: 12px; margin-top: 3px; }
  .owe i, .sq { display: block; border-radius: 3px; }
  .owe .debt, .sq.debt { background: repeating-linear-gradient(-45deg, var(--g-debt) 0 3px, transparent 3px 6px); box-shadow: inset 0 0 0 1px var(--g-debt); }
  .owe .left, .sq.left { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .lg .sq { display: inline-block; width: 12px; height: 9px; border-radius: 2px; }
  .dl { color: var(--g-debt); }
  .nl { color: var(--accent); }
  .split i { display: block; }
  .lg span { white-space: nowrap; }
  .lg { display: flex; gap: 6px 16px; font-size: 13px; color: var(--text-2); margin-top: 8px; flex-wrap: wrap; }
  .lg i { display: inline-block; width: 9px; height: 9px; border-radius: 50%; margin-right: 6px; }
  .k .v { font-family: var(--figures); font-size: 26px; margin-top: 4px; }
  .k .s { font-size: 12px; color: var(--text-3); }
  @media (max-width: 760px) {
    .wall { padding: 20px; height: auto; min-height: 100vh; }
    .top { flex-wrap: wrap; }
    .clock { font-size: 16px; }
    .foot { grid-template-columns: 1fr 1fr; }
    .chart { height: 240px; }
  }
</style>
