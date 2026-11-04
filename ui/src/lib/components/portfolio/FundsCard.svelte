<script lang="ts">
  // Look-through › Your funds: each fund you hold, what it is made of (a family bar in the
  // treemap's colours), how many names it really spreads across, and its closest twin among
  // your other funds. File dates only show up when something is off.
  import type { Company, FundStatus } from "../../api";
  import { formatDate, formatPct } from "../../format/money";
  import { closestTwin, familyHue, fundMix, FLAG, type Pair } from "../../portfolio/lookthrough";
  import Money from "../amounts/Money.svelte";
  import { hashFor } from "../../stores/route.svelte";

  let { funds, companies, pairs, total, currency, asOf }: { funds: FundStatus[]; companies: Company[]; pairs: Pair[]; total: number; currency: string; asOf?: string } = $props();
  const STALE_DAYS = 45;
  const rows = $derived(funds.map((f) => ({
    f,
    mix: fundMix(companies, f.symbol),
    twin: closestTwin(pairs, f.symbol),
    weight: total > 0 ? (Number(f.value) / total) * 100 : 0,
    stale: !!(f.as_of && asOf && (new Date(asOf).getTime() - new Date(f.as_of).getTime()) / 86400000 > STALE_DAYS),
  })));
  const fam = (name: string) => `var(${familyHue(name as never)})`;
  const neutral = (h: string) => h === "--f-none";
</script>

<section class="card funds" aria-label="Your funds">
  <div class="ch">
    <h2 class="label">Your funds</h2>
    <span class="key faint"><span>what's inside, by sector family</span></span>
  </div>
  <div class="head" aria-hidden="true"><span>Fund</span><span class="r">Your value</span><span>Inside</span><span class="r" title="1 ÷ the sum of squared weights inside the fund">Acts like</span></div>
  {#each rows as r (r.f.symbol)}
    <div class="row">
      <div class="id">
        <a class="sym" href={hashFor({ name: "security", symbol: r.f.symbol })}>{r.f.symbol}</a>
        <span class="sub">
          {#if r.f.name}{r.f.name}{/if}
          {#if r.f.children.length}&nbsp;· holds {r.f.children.join(", ")}{/if}
        </span>
        {#if r.twin && r.twin.pct >= FLAG.pair}<span class="twin" class:hot={r.twin.pct >= 80}>{r.twin.pct.toFixed(0)}% the same as {r.twin.other}</span>{/if}
        {#if !r.f.resolved}<span class="twin hot">no holdings file, counted as one name</span>
        {:else if r.stale}<span class="twin hot">file from {formatDate(r.f.as_of!)}</span>{/if}
        {#if r.f.coverage_pct != null && Number(r.f.coverage_pct) < 99.5}<span class="twin hot">{r.f.coverage_pct}% of it disclosed</span>{/if}
        {#if r.f.unresolved_children.length}<span class="twin hot">no file for {r.f.unresolved_children.join(", ")}</span>{/if}
      </div>
      <div class="val r">
        <Money value={r.f.value} {currency} />
        <span class="wt" title="{formatPct(r.weight)} of your investments"><i style="width:{r.weight}%"></i></span>
        <span class="sub num">{formatPct(r.weight)}</span>
      </div>
      <div class="mix" title={r.mix.map((m) => `${m.family} ${formatPct(m.share * 100, 0)}`).join(" · ")}>
        {#if r.mix.length}
          <span class="stack">{#each r.mix as m (m.family)}<i style="width:{m.share * 100}%;background:{fam(m.family)}" class:soft={neutral(familyHue(m.family))}></i>{/each}</span>
          <span class="sub">{r.mix.slice(0, 2).map((m) => `${m.family.split(" ")[0].replace(",", "")} ${formatPct(m.share * 100, 0)}`).join(" · ")}</span>
        {:else}<span class="sub">—</span>{/if}
      </div>
      <div class="acts r">
        {#if r.f.effective_holdings}<span class="num">~{r.f.effective_holdings}</span><span class="sub">names</span>{:else}<span class="faint">—</span>{/if}
      </div>
    </div>
  {/each}
</section>

<style>
  .ch { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; margin-bottom: 6px; }
  .ch h2 { margin: 0; font-weight: 500; }
  .key { font-size: 12px; }
  .head, .row { display: grid; grid-template-columns: minmax(0, 1.6fr) 150px minmax(120px, 1fr) 72px; gap: 18px; }
  .head { font-size: 12px; color: var(--text-3); padding: 4px 0; }
  .row { padding: 10px 0; border-top: 1px solid var(--border); align-items: start; font-size: 13px; }
  .r { text-align: right; }
  .id { min-width: 0; }
  .sym { font-weight: 500; color: var(--text); text-decoration: none; }
  .sym:hover { color: var(--accent); }
  .sub { display: block; font-size: 11px; color: var(--text-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .twin { display: block; font-size: 11px; color: var(--text-2); margin-top: 2px; }
  .twin.hot { color: var(--warn); }
  .val { display: grid; grid-template-columns: 1fr; justify-items: end; }
  .wt { display: block; width: 100%; max-width: 120px; height: 5px; background: var(--surface-2); border-radius: 3px; overflow: hidden; margin: 5px 0 2px; }
  .wt i { display: block; height: 100%; background: var(--fill); margin-left: auto; }
  .mix { min-width: 0; padding-top: 4px; }
  .stack { display: flex; gap: 1px; height: 10px; border-radius: 3px; overflow: hidden; background: var(--surface-2); margin-bottom: 4px; }
  .stack i { display: block; height: 100%; }
  .stack i.soft { background: var(--fill-soft) !important; }
  @media (max-width: 700px) {
    .head { display: none; }
    .row { grid-template-columns: minmax(0, 1fr) auto; gap: 6px 14px; }
    .mix { grid-column: 1; }
    .acts { grid-column: 2; display: flex; gap: 4px; align-items: baseline; justify-content: flex-end; padding-top: 4px; }
    .acts .sub { display: inline; }
  }
</style>
