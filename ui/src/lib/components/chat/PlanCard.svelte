<script lang="ts">
  // One proposal, compact: a header line (what, when, effect on net worth, actions)
  // over one line per change (now → proposed, signed delta, a diverging bar for its
  // effect). The words a row came from show on hover. A proposal with nothing to
  // apply is a single red line; a decided one collapses to a single line.
  import type { Plan } from "../../api";
  import { editable, editPlan, fmtUnits, hasWork, onlySpends, rowHint, rowTitle, spendOf } from "../../chat";
  import { formatDate, formatPlain } from "../../format/money";
  import Money from "../amounts/Money.svelte";
  import DeltaBar from "../amounts/DeltaBar.svelte";

  let {
    plan,
    base,
    rates,
    title = null,
    badge = null,
    meta = "",
    locked = false,
    status = "open",
    outcome = null,
    approveText = "Approve",
    cancelText = "Cancel",
    onapprove,
    oncancel,
    onedit,
    now = null,
    labels = {},
  }: {
    plan: Plan;
    base: string;
    rates: Record<string, string>;
    title?: string | null;
    badge?: string | null;
    meta?: string;
    /** Today's net worth. The plan's own `net_worth_after` was worked out when it was
     * proposed and goes stale once anything else is saved, so the card adds to this. */
    now?: string | null;
    locked?: boolean;
    status?: "open" | "approved" | "cancelled" | "failed";
    outcome?: string | null;
    approveText?: string;
    cancelText?: string;
    onapprove: (plan: Plan) => void;
    oncancel: () => void;
    onedit?: (plan: Plan) => void;
    /** Category id -> label, for purchases. */
    labels?: Record<string, string>;
  } = $props();

  let editing = $state(false);
  let edits = $state<Record<number, string>>({});
  const draft = $derived(editing ? editPlan(plan, edits, rates, base) : plan);
  const shown = $derived(draft ?? plan);
  const maxEffect = $derived(Math.max(1, ...shown.rows.map((r) => Math.abs(Number(r.effect ?? 0)))));
  const canApprove = $derived(status === "open" && !locked && draft != null && hasWork(shown));
  const budgetOnly = $derived(onlySpends(shown));
  const n = $derived(shown.rows.length);
  const after = $derived(
    now != null && shown.net_change != null ? String(Number(now) + Number(shown.net_change)) : shown.net_worth_after,
  );
  const heading = $derived(title ?? (budgetOnly ? `${n} ${n === 1 ? "purchase" : "purchases"}` : `${n} ${n === 1 ? "change" : "changes"}`));
  const why = $derived(
    locked ? "The ledger file has an error; fix it first" : !hasWork(plan) ? "Nothing here can be applied" : "Nothing is written until you approve",
  );

  function fig(v: string | null, kind: string) {
    if (v == null) return "—";
    return kind === "trade" ? fmtUnits(Number(v)) : formatPlain(v);
  }
  /** The faint word after a row's name: what the figures are, not where they came from. */
  function tag(r: Plan["rows"][number]): string {
    if (r.kind === "trade") return r.price ? `@ ${Number(r.price).toFixed(2)}` : "no price";
    if (r.kind === "price") return r.currency;
    if (r.kind === "new_account") return "new";
    if (r.kind === "spend") {
      const s = spendOf(shown, shown.rows.indexOf(r));
      const cat = s?.category ? (labels[s.category] ?? s.category[0].toUpperCase() + s.category.slice(1)) : null;
      // "Coffee · Coffee · Cash" says the same thing twice; drop the category then.
      const same = cat != null && cat.toLowerCase() === (r.detail ?? "").toLowerCase();
      return [same ? null : cat, r.account_name].filter(Boolean).join(" · ");
    }
    return r.liability ? "owed" : "";
  }
  function approve() {
    if (!canApprove || !draft) return;
    if (editing) onedit?.(draft);
    onapprove(draft);
    editing = false;
  }
</script>

{#if n === 0}
  {#each plan.rejected as x}
    <div class="line rej"><span class="x" aria-hidden="true">✕</span><span>{x.reason}{#if x.hint}&nbsp;<span class="faint">· {x.hint}</span>{/if}</span></div>
  {/each}
{:else if status === "approved" || status === "cancelled"}
  <div class="line" class:ok={status === "approved"}>
    <span class="x" aria-hidden="true">{status === "approved" ? "✓" : "–"}</span>
    <span class:faint={status === "cancelled"}>{outcome ?? status}</span>
    {#if status === "approved" && shown.net_change != null && !budgetOnly}<span class="muted">· net worth <Money value={shown.net_change} currency={base} signed tone="delta" /></span>{/if}
  </div>
{:else}
  <div class="plan card" class:failed={status === "failed"}>
    <div class="top">
      {#if badge}<span class="pill" class:mcp={badge === "MCP"}>{badge}</span>{/if}
      <b>{heading}</b>
      <span class="faint">{meta || formatDate(plan.date)}</span>
      {#if budgetOnly}
        <span class="faint net" title="A purchase counts toward your budget and the game; it never changes a balance">budget only · net worth unchanged</span>
      {:else if shown.net_change != null}
        <span class="muted net">net worth <Money value={shown.net_change} currency={base} signed tone="delta" />{#if after != null}&nbsp;→ <b><Money value={after} currency={base} /></b>{/if}</span>
      {:else}
        <span class="faint net" title="A row has no price or FX rate, so the total effect is not known">effect unknown</span>
      {/if}
      <span class="sp"></span>
      {#if status === "open"}
        <span class="acts">
        <button type="button" class="btn" onclick={approve} disabled={!canApprove} title={why}>{approveText}</button>
        {#if onedit && !budgetOnly}
          {#if editing}<button type="button" class="link" onclick={() => (editing = false)}>Undo edits</button>{:else}<button type="button" class="link" onclick={() => { edits = {}; editing = true; }} disabled={locked}>Edit</button>{/if}
        {/if}
        <button type="button" class="link" onclick={oncancel}>{cancelText}</button>
        </span>
      {/if}
    </div>
    {#each shown.rows as r, i}
      {@const d = r.delta == null ? null : Number(r.delta)}
      <div class="row" class:same={d === 0} title={rowHint(r)}>
        <span class="name">{rowTitle(r)}{#if r.kind === "note"}<small>{r.detail}</small>{:else if tag(r)}<small>{tag(r)}</small>{/if}</span>
        <span class="r num faint">{r.kind === "note" || r.kind === "spend" ? "" : fig(r.from, r.kind)}</span>
        <span class="arr" aria-hidden="true">{r.kind === "note" || r.kind === "spend" ? "" : "→"}</span>
        <span class="r num">
          {#if editing && editable(r)}
            <input type="text" inputmode="decimal" aria-label="{rowTitle(r)} proposed" placeholder={fig(r.to, r.kind)} value={edits[i] ?? ""} oninput={(e) => (edits[i] = e.currentTarget.value)} class:bad={draft == null && (edits[i] ?? "") !== ""} />
          {:else}
            {r.kind === "note" ? "" : fig(r.to, r.kind)}
          {/if}
        </span>
        <span class="r num" class:up={d != null && (r.liability ? d < 0 : d > 0)} class:down={d != null && (r.liability ? d > 0 : d < 0)} class:faint={d == null || d === 0}>
          {#if r.kind === "spend"}<span class="faint tb">to the budget</span>{:else if d == null}{""}{:else if d === 0}same{:else}{d > 0 ? "+" : "−"}{r.kind === "trade" ? fmtUnits(Math.abs(d)) : formatPlain(Math.abs(d))}{#if r.kind === "trade"}<span class="u">&nbsp;units</span>{/if}{/if}
        </span>
        <span class="eff">{#if r.effect != null && Number(r.effect) !== 0}<DeltaBar value={Number(r.effect)} max={maxEffect} width={56} />{/if}</span>
      </div>
    {/each}
    {#each plan.rejected as x}
      <div class="row rejrow"><span class="x" aria-hidden="true">✕</span><span>{x.reason}{#if x.hint}<span class="faint"> · {x.hint}</span>{/if}</span></div>
    {/each}
    {#if status === "failed" && outcome}<div class="row rejrow"><span class="x" aria-hidden="true">✕</span><span>{outcome}</span></div>{/if}
  </div>
{/if}

<style>
  .plan { container-type: inline-size; display: flex; flex-direction: column; padding: 10px 14px; border-radius: 12px 12px 12px 2px; }
  .plan.failed { border-color: var(--negative); }
  .top { display: flex; align-items: center; gap: 10px; padding-bottom: 6px; flex-wrap: wrap; }
  .top b { font-weight: 600; }
  .net { font-size: 13px; }
  .sp { flex: 1; }
  .acts { display: flex; align-items: center; gap: 10px; }
  .pill { font-size: 12px; padding: 1px 6px; border-radius: 9px; background: var(--surface-2); color: var(--text-2); font-weight: 600; }
  .pill.mcp { background: color-mix(in oklab, var(--g-cash) 18%, transparent); color: var(--g-cash); }
  .row { display: grid; grid-template-columns: minmax(0, 1fr) 96px 14px 104px 116px 56px; gap: 10px; align-items: center; padding: 5px 0; border-top: 1px solid var(--border); font-size: 13px; }
  .row.same { opacity: 0.6; }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { color: var(--text-3); font-size: 12px; margin-left: 6px; }
  .arr { color: var(--text-3); text-align: center; }
  .r { text-align: right; }
  .u { font-size: 11px; color: var(--text-3); }
  .tb { font-family: var(--font); font-size: 12px; }
  .eff { display: flex; justify-content: flex-end; }
  .up { color: var(--positive); }
  .down { color: var(--negative); }
  .rejrow { grid-template-columns: 14px minmax(0, 1fr); }
  .line { display: flex; gap: 8px; align-items: baseline; font-size: 13px; align-self: flex-start; color: var(--text-2); }
  .line.ok .x { color: var(--positive); }
  .x { color: var(--negative); font-weight: 600; }
  .rej { color: var(--text); }
  input { width: 100%; text-align: right; background: var(--bg); border: 1px solid var(--text-3); border-radius: 6px; padding: 2px 6px; color: var(--text); font: inherit; font-family: var(--figures); }
  input.bad { border-color: var(--negative); }
  input:focus-visible { outline: 2px solid var(--text-2); outline-offset: -1px; }
  .btn { background: var(--text); color: var(--bg); border: 0; border-radius: 7px; padding: 5px 12px; font: inherit; font-size: 13px; font-weight: 600; cursor: pointer; white-space: nowrap; }
  .btn:disabled { opacity: 0.4; cursor: default; }
  .link { color: var(--text-2); background: none; border: 0; font: inherit; font-size: 13px; padding: 0; cursor: pointer; text-decoration: underline; text-decoration-color: var(--border); text-underline-offset: 3px; }
  .link:hover { color: var(--text); }
  .link:disabled { color: var(--text-3); cursor: default; }
  button:focus-visible { outline: 2px solid var(--text-2); outline-offset: 2px; }
  @container (max-width: 640px) {
    .row { grid-template-columns: minmax(0, 1fr) 96px 104px; }
    .row > :nth-child(2), .row > :nth-child(3), .row > :nth-child(6) { display: none; }
    .rejrow { grid-template-columns: 14px minmax(0, 1fr); }
    .sp { flex-basis: 100%; height: 0; }
    .acts { margin-left: auto; }
  }
</style>
