<script lang="ts">
  // Look-through › Exposure: one donut per question (country, sector, currency, asset class),
  // the leader in the middle, the next two under it. "All" opens the full list in a box that
  // scrolls. Region is left out: it repeats country. Through funds or as held.
  import type { Bucket, LookThroughView } from "../../api";
  import { formatMoney, formatPct } from "../../format/money";
  import { compactMoney, familyHue, familyOf } from "../../portfolio/lookthrough";
  import Donut from "../../charts/Donut.svelte";

  let { lt }: { lt: LookThroughView } = $props();
  const cur = $derived(lt.base_currency);
  let mode = $state<"lookthrough" | "as_held">("lookthrough");
  const anyResolved = $derived(lt.lookthrough.funds.some((f) => f.resolved));
  const x = $derived(mode === "lookthrough" && anyResolved ? lt.lookthrough : lt.as_held);
  const sets = $derived<{ title: string; rows: Bucket[] }[]>([
    { title: "Country", rows: x.by_country },
    { title: "Sector", rows: x.by_sector },
    { title: mode === "lookthrough" ? "Currency underneath" : "Currency traded in", rows: x.by_currency },
    { title: "Asset class", rows: x.by_asset_class },
  ]);
  let open = $state<string | null>(null);
  // Sectors take the treemap's family colours; other questions step down in grey.
  const GREYS = ["var(--fill)", "color-mix(in oklab, var(--fill) 70%, var(--surface-2))", "color-mix(in oklab, var(--fill) 45%, var(--surface-2))", "var(--fill-soft)"];
  const color = (title: string, r: Bucket, i: number) => {
    if (title === "Sector" && r.slot) {
      const h = familyHue(familyOf(r.slot));
      if (h !== "--f-none") return `var(${h})`;
    }
    return GREYS[Math.min(i, GREYS.length - 1)];
  };
  // The top four as slices, the rest as one quiet slice.
  const slices = (title: string, rows: Bucket[]) => {
    const rest = rows.slice(4).reduce((s, r) => s + Number(r.pct), 0);
    return [
      ...rows.slice(0, 4).map((r, i) => ({ label: r.label, value: Number(r.pct), color: color(title, r, i) })),
      ...(rest > 0 ? [{ label: `${rows.length - 4} others`, value: rest, color: "var(--surface-2)" }] : []),
    ];
  };
  const short = (s: string) => (s.length > 13 ? s.slice(0, 12) + "…" : s);
  const muted = (label: string) => label === "Undisclosed" || label === "Unknown" || label === "Unclassified" || label.endsWith("others");
</script>

<section class="card exp" aria-label="Exposure">
  <div class="ch">
    <h2 class="label">Exposure</h2>
    <div class="toggle" role="radiogroup" aria-label="Count funds">
      <button type="button" role="radio" aria-checked={mode === "lookthrough"} class:on={mode === "lookthrough"} onclick={() => (mode = "lookthrough")} disabled={!anyResolved} title={anyResolved ? "Follow each fund into what it owns" : "No holdings files yet"}>Look-through</button>
      <button type="button" role="radio" aria-checked={mode === "as_held"} class:on={mode === "as_held"} onclick={() => (mode = "as_held")} title="Count each fund as one line">As held</button>
    </div>
  </div>
  <div class="grid">
    {#each sets as s (s.title)}
      {@const top = s.rows[0]}
      {#if top}
        <div class="q">
          <span class="t">{s.title}</span>
          <Donut size={120} hole={0.66} center={[`${Math.round(Number(top.pct))}%`, short(top.label)]} label={s.title} slices={slices(s.title, s.rows)} />
          <div class="nx">
            {#each s.rows.slice(1, 3) as r, i (r.label)}<span><i class="dot" style="background:{color(s.title, r, i + 1)}"></i>{r.label} <b class="num">{formatPct(r.pct)}</b></span>{/each}
          </div>
          {#if s.rows.length > 1}<button type="button" class="all" aria-expanded={open === s.title} onclick={() => (open = open === s.title ? null : s.title)}>{open === s.title ? "Hide" : `All ${s.rows.length}`}</button>{/if}
        </div>
      {/if}
    {/each}
  </div>
  {#each sets as s (s.title)}
    {#if open === s.title}
      {@const max = Math.max(...s.rows.map((r) => Number(r.pct)), 0.01)}
      <div class="list" aria-label="{s.title}, every bucket">
        <div class="lh label">{s.title}</div>
        {#each s.rows as r, i (r.label)}
          <div class="row" class:faint={muted(r.label)} title="{r.label}: {formatMoney(r.value, cur)} ({r.pct}%)">
            <span class="name">{r.label}</span>
            <span class="track"><i style="width:{(Number(r.pct) / max) * 100}%;background:{color(s.title, r, Math.min(i, 2))}"></i></span>
            <span class="pct num">{formatPct(r.pct)}</span>
            <span class="amt money">{compactMoney(r.value)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/each}
  <p class="faint note">
    {#if mode === "lookthrough"}Each fund followed into what it owns, up to three levels deep. "Undisclosed" is the part of a fund its file does not list.{:else}Each position as one line: a fund counts as a fund, in the currency and country it trades in.{/if}
  </p>
</section>

<style>
  .ch { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; margin-bottom: 4px; }
  .ch h2 { margin: 0; font-weight: 500; }
  .toggle { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
  .toggle button { padding: 4px 10px; font: inherit; font-size: 12px; color: var(--text-2); background: none; border: 0; cursor: pointer; }
  .toggle button.on { background: var(--accent-soft); color: var(--text); }
  .toggle button:disabled { opacity: 0.5; cursor: default; }
  .toggle button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 170px), 1fr)); gap: 18px; padding: 12px 0; border-top: 1px solid var(--border); }
  .q { display: flex; flex-direction: column; align-items: center; gap: 8px; min-width: 0; }
  .t { font-size: 12px; color: var(--text-3); }
  .nx { display: flex; flex-wrap: wrap; justify-content: center; gap: 2px 12px; font-size: 12px; color: var(--text-2); text-align: center; }
  .nx b { color: var(--text); font-weight: 500; }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; }
  .all { background: none; border: 0; padding: 0; font: inherit; font-size: 12px; color: var(--text-2); cursor: pointer; text-decoration: underline; text-underline-offset: 3px; }
  .all:hover { color: var(--text); }
  .all:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .list { border-top: 1px solid var(--border); padding-top: 10px; max-height: 240px; overflow-y: auto; overscroll-behavior: contain; padding-right: 4px; }
  .lh { margin-bottom: 6px; font-weight: 500; }
  .row { display: grid; grid-template-columns: minmax(80px, 160px) minmax(0, 1fr) 48px 56px; gap: 10px; align-items: center; font-size: 12px; padding: 3px 0; }
  .row .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .track { height: 8px; background: var(--surface-2); border-radius: 3px; overflow: hidden; position: relative; }
  .track i { position: absolute; left: 0; top: 0; bottom: 0; min-width: 1px; }
  .pct, .amt { text-align: right; }
  .amt { color: var(--text-2); }
  .note { font-size: 12px; margin: 10px 0 0; }
  @media (max-width: 600px) {
    .row { grid-template-columns: 84px minmax(0, 1fr) 44px; }
    .amt { display: none; }
  }
</style>
