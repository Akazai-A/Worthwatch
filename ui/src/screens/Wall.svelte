<script lang="ts">
  // Full-screen, read-only view for a spare tablet or a second monitor: the number,
  // the line with the next 12 months dashed, the asset split and three figures to
  // glance at. No rail. Updates live through the same event stream as every screen.
  import { onMount } from "svelte";
  import { GROUPS, GROUP_LABEL, type LedgerView } from "../lib/api";
  import { formatCompact, formatDate, formatMoney, formatSigned } from "../lib/money";
  import { monthYear, yearsBetween } from "../lib/plan";
  import { plan } from "../lib/stores/plan.svelte";
  import { prefs } from "../lib/stores/prefs.svelte";
  import FanChart from "../lib/charts/FanChart.svelte";

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
  const hide = (s: string) => (prefs.blur ? "•••" : s);
  let h = $state(400);
  const clock = $derived(
    `${now.toLocaleDateString("en-CA", { weekday: "short", month: "short", day: "numeric" })} · ${now.toLocaleTimeString("en-CA", { hour: "2-digit", minute: "2-digit", hour12: false })}`,
  );
</script>

<div class="wall" class:blur={prefs.blur}>
  <div class="top"><a href="#/" title="Back to the app">worthwatch</a><span>net worth · {cur} · as of {formatDate(view.as_of)}</span><span class="clock">{clock}</span></div>
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
      <div class="split">{#each GROUPS.filter((g) => g !== "debt") as g}<i style="width:{share(g)}%;background:var(--g-{g})"></i>{/each}</div>
      <div class="lg">
        {#each GROUPS.filter((g) => g !== "debt") as g}<span><i style="background:var(--g-{g})"></i>{GROUP_LABEL[g]} {share(g).toFixed(0)}%</span>{/each}
        <span><i style="background:var(--g-debt)"></i>Debt {assets > 0 ? ((Number(view.totals.liabilities) / assets) * 100).toFixed(0) : "–"}% of assets</span>
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
  .big { font-size: clamp(64px, 10vw, 128px); letter-spacing: -0.03em; line-height: 1; }
  .chips { display: flex; gap: 28px; font-family: var(--figures); font-size: clamp(16px, 1.8vw, 22px); margin-top: 12px; color: var(--text-2); flex-wrap: wrap; }
  .chips b { font-weight: 400; color: var(--positive); }
  .chips b.down { color: var(--negative); }
  .chart { min-height: 160px; overflow: hidden; }
  .foot { display: grid; grid-template-columns: 2fr 1fr 1fr 1fr; gap: 28px; align-items: end; }
  .split { display: flex; height: 12px; border-radius: 4px; overflow: hidden; }
  .split i { display: block; }
  .lg span { white-space: nowrap; }
  .lg { display: flex; gap: 6px 16px; font-size: 13px; color: var(--text-2); margin-top: 8px; flex-wrap: wrap; }
  .lg i { display: inline-block; width: 9px; height: 9px; border-radius: 50%; margin-right: 6px; }
  .k .v { font-family: var(--figures); font-size: 26px; margin-top: 4px; }
  .k .s { font-size: 12px; color: var(--text-3); }
  @media (max-width: 760px) {
    .wall { padding: 20px; height: auto; min-height: 100vh; }
    .foot { grid-template-columns: 1fr 1fr; }
    .chart { height: 240px; }
  }
</style>
