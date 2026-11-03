<script lang="ts">
  // One proposal as a diff card: a row per change (now → proposed, signed delta,
  // a diverging bar for its effect on net worth), rejections in red, and the
  // decision buttons. Thin: the plan comes in, the decision goes out.
  import type { Plan } from "../api";
  import { approveLabel, editable, editPlan, fmtUnits, rowHint, rowTitle, rowUnit } from "../chat";
  import { formatDate, formatPlain } from "../money";
  import Money from "./Money.svelte";
  import DeltaBar from "./DeltaBar.svelte";

  let {
    plan,
    base,
    rates,
    title = "Proposal",
    badge = null,
    meta = "",
    locked = false,
    status = "open",
    outcome = null,
    approveText = null,
    cancelText = "Cancel",
    onapprove,
    oncancel,
    onedit,
  }: {
    plan: Plan;
    base: string;
    rates: Record<string, string>;
    title?: string;
    badge?: string | null;
    meta?: string;
    locked?: boolean;
    status?: "open" | "approved" | "cancelled" | "failed";
    outcome?: string | null;
    approveText?: string | null;
    cancelText?: string;
    onapprove: (plan: Plan) => void;
    oncancel: () => void;
    onedit?: (plan: Plan) => void;
  } = $props();

  let editing = $state(false);
  let edits = $state<Record<number, string>>({});
  const draft = $derived(editing ? editPlan(plan, edits, rates, base) : plan);
  const shown = $derived(draft ?? plan);
  const maxEffect = $derived(Math.max(1, ...shown.rows.map((r) => Math.abs(Number(r.effect ?? 0)))));
  const canApprove = $derived(status === "open" && !locked && draft != null && shown.mutations.length > 0);
  const open = $derived(status === "open");

  function fig(v: string | null, kind: string) {
    if (v == null) return "—";
    return kind === "trade" ? fmtUnits(Number(v)) : formatPlain(v);
  }
  function startEdit() {
    edits = {};
    editing = true;
  }
  function approve() {
    if (!canApprove || !draft) return;
    if (editing) onedit?.(draft);
    onapprove(draft);
    editing = false;
  }
</script>

<div class="plan card" class:done={!open} class:rejected={plan.rows.length === 0 && plan.rejected.length > 0}>
  <div class="top">
    <span class="what">
      {#if plan.rows.length === 0 && plan.rejected.length}<b class="bad">Rejected</b>{:else}<b>{title}</b>{/if}
      {#if badge}<span class="pill" class:mcp={badge === "MCP"}>{badge}</span>{/if}
      {#if plan.rows.length}<span class="muted">· {plan.rows.length} {plan.rows.length === 1 ? "change" : "changes"} · dated {formatDate(plan.date)}</span>{/if}
      {#if meta}<span class="muted">· {meta}</span>{/if}
    </span>
    {#if shown.net_change != null && shown.rows.length}
      <span class="net muted">net worth <Money value={shown.net_change} currency={base} signed tone="delta" />{#if shown.net_worth_after != null} → <b><Money value={shown.net_worth_after} currency={base} /></b>{/if}</span>
    {:else if shown.rows.length}
      <span class="net faint" title="A row has no price or no FX rate, so the total effect is not known">net effect unknown</span>
    {/if}
  </div>

  {#if shown.rows.length}
    <div class="row h"><span>Change</span><span class="r">Now</span><span class="r">Proposed</span><span class="r">Delta</span><span class="r">Effect</span></div>
    {#each shown.rows as r, i}
      {@const d = r.delta == null ? null : Number(r.delta)}
      <div class="row" class:same={d === 0}>
        <span class="name">
          <span class="t">{rowTitle(r)}</span>
          {#if r.kind === "note"}<span class="hint">{r.detail}</span>{:else if rowHint(r)}<span class="hint">{rowHint(r)}</span>{/if}
        </span>
        <span class="r num muted">{r.kind === "note" ? "" : fig(r.from, r.kind)}</span>
        <span class="r num">
          {#if editing && editable(r)}
            <input type="text" inputmode="decimal" aria-label="{rowTitle(r)} proposed" placeholder={fig(r.to, r.kind)} value={edits[i] ?? ""} oninput={(e) => (edits[i] = e.currentTarget.value)} class:bad={draft == null && (edits[i] ?? "") !== ""} />
          {:else}
            {r.kind === "note" ? "" : fig(r.to, r.kind)}
          {/if}
        </span>
        <span class="r num d" class:up={d != null && (r.liability ? d < 0 : d > 0)} class:down={d != null && (r.liability ? d > 0 : d < 0)} class:faint={d === 0 || d == null}>
          {#if d == null}{r.kind === "new_account" ? "new" : ""}{:else if d === 0}same{:else}{d > 0 ? "+" : "−"}{r.kind === "trade" ? fmtUnits(Math.abs(d)) : formatPlain(Math.abs(d))}{#if rowUnit(r)}<span class="u">&nbsp;{rowUnit(r)}</span>{/if}{/if}
        </span>
        <span class="r eff">
          {#if r.effect != null && Number(r.effect) !== 0}<DeltaBar value={Number(r.effect)} max={maxEffect} width={64} />{:else if r.effect == null && r.kind !== "note"}<span class="faint small" title="No price or FX rate to value it">no price</span>{/if}
        </span>
      </div>
    {/each}
  {/if}

  {#each plan.rejected as x}
    <div class="rej">
      <span class="bad">{x.reason}</span>
      {#if x.source && plan.rows.length}<span class="faint"> · “{x.source}”</span>{/if}
      {#if x.hint}<span class="hint block">{x.hint}</span>{/if}
    </div>
  {/each}

  {#if plan.rows.length}
    <div class="acts">
      {#if open}
        <button type="button" class="btn" onclick={approve} disabled={!canApprove}>{approveText ?? approveLabel(shown)}</button>
        {#if onedit && !editing}<button type="button" class="link" onclick={startEdit} disabled={locked}>Edit</button>{/if}
        {#if editing}<button type="button" class="link" onclick={() => (editing = false)}>Undo edits</button>{/if}
        <button type="button" class="link" onclick={oncancel}>{cancelText}</button>
        <span class="hint">
          {#if locked}The ledger file has an error; fix it first.{:else if plan.mutations.length === 0}Nothing here can be applied.{:else if editing}Type the figure you want; empty keeps the proposal.{:else}Nothing is written until you approve.{/if}
        </span>
      {:else}
        <span class="hint" class:ok={status === "approved"} class:bad={status === "failed"}>{outcome ?? status}</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .plan { display: flex; flex-direction: column; gap: 2px; padding: 12px 14px; border-radius: 12px 12px 12px 2px; }
  .plan.rejected { border-color: var(--negative); }
  .plan.done { opacity: 0.72; }
  .top { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; padding-bottom: 6px; }
  .what b { font-weight: 600; }
  .net { font-size: 12px; }
  .pill { font-size: 10px; padding: 1px 6px; border-radius: 9px; background: var(--accent-soft); color: var(--accent); font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; margin-left: 4px; }
  .pill.mcp { background: color-mix(in oklab, var(--g-cash) 18%, transparent); color: var(--g-cash); }
  .row { display: grid; grid-template-columns: minmax(0, 1fr) 110px 120px 130px 72px; gap: 12px; align-items: center; padding: 7px 0; border-top: 1px solid var(--border); font-size: 13px; }
  .row.h { border-top: 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-3); padding: 0 0 4px; }
  .row.same { opacity: 0.6; }
  .name { min-width: 0; display: flex; flex-direction: column; }
  .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint { font-size: 12px; color: var(--text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint.block { display: block; white-space: normal; }
  .r { text-align: right; }
  .u { font-size: 11px; color: var(--text-3); }
  .eff { display: flex; justify-content: flex-end; }
  .up { color: var(--positive); }
  .down { color: var(--negative); }
  .bad { color: var(--negative); }
  .ok { color: var(--positive); }
  .small { font-size: 11px; }
  .rej { font-size: 13px; padding: 6px 0 0; }
  .acts { display: flex; gap: 14px; align-items: center; padding-top: 10px; flex-wrap: wrap; }
  .acts .hint { white-space: normal; }
  input { width: 100%; text-align: right; background: var(--bg); border: 1px solid var(--accent); border-radius: 7px; padding: 4px 8px; color: var(--text); font: inherit; font-family: var(--figures); }
  input.bad { border-color: var(--negative); }
  input:focus-visible { outline: 2px solid var(--accent); outline-offset: -1px; }
  .btn { background: var(--accent); color: var(--bg); border: 0; border-radius: 8px; padding: 8px 14px; font: inherit; font-weight: 600; cursor: pointer; white-space: nowrap; }
  .btn:disabled { opacity: 0.45; cursor: default; }
  .link { color: var(--accent); background: none; border: 0; font: inherit; cursor: pointer; font-size: 13px; padding: 0; }
  .link:disabled { color: var(--text-3); cursor: default; }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (max-width: 700px) {
    .row { grid-template-columns: minmax(0, 1fr) 90px 100px; }
    .row > :nth-child(2), .row > :nth-child(5) { display: none; }
  }
</style>
