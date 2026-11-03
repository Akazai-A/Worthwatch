<script lang="ts">
  // Paste a broker holdings export for one position account; preview the diff; apply.
  import { apply, ApiError, parseHoldings, type AccountView, type PastePlan } from "../api";
  import { formatPlain, formatUnits, MASK, amountsHidden } from "../money";
  import { view as viewStore } from "../stores/view.svelte";

  let { account, date, locked = false, onclose }: { account: AccountView; date: string; locked?: boolean; onclose: () => void } = $props();
  let text = $state("");
  let plan = $state<PastePlan | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  const changes = $derived(plan ? plan.rows.filter((r) => Number(r.delta) !== 0).length : 0);
  const prices = $derived(plan ? plan.rows.filter((r) => r.price != null).length : 0);
  const suffix = $derived(account.currency === "CAD" ? ".TO" : "");

  function schedule() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void preview(), 250);
  }

  async function preview() {
    const t = text.trim();
    if (!t) { plan = null; return; }
    try {
      plan = await parseHoldings(account.id, date, t);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function applyPlan() {
    if (!plan || !plan.mutations.length || busy) return;
    busy = true;
    error = null;
    try {
      viewStore.set(await apply(plan.mutations));
      onclose();
    } catch (e) {
      error = e instanceof ApiError && e.line ? `line ${e.line}: ${e.message}` : e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="paste" id="paste-{account.id}">
  <p class="muted hint">
    Paste the holdings table from Wealthsimple or Questrade (select the table, copy, paste here; a CSV export works too).
    Symbols are matched to this account{#if suffix}; a bare ticker gets <code>{suffix}</code>{/if}. Nothing is written until you apply.
  </p>
  <textarea bind:value={text} oninput={schedule} placeholder={"XEQT\tiShares Core Equity ETF Portfolio\t426.5 shares\t$34.12\t$14,552.18"} aria-label="Pasted holdings for {account.name}" disabled={locked}></textarea>

  {#if plan}
    {#if plan.rows.length}
      <div class="prev h"><span>Symbol</span><span class="r">Now</span><span class="r">Pasted</span><span class="r">Change</span><span class="r">Price</span></div>
      {#each plan.rows as r (r.symbol)}
        {@const d = Number(r.delta)}
        <div class="prev">
          <span>{r.symbol}{#if r.new}<span class="new">new</span>{:else if r.pasted_symbol.toUpperCase() !== r.symbol}<span class="faint small">&nbsp;from {r.pasted_symbol}</span>{/if}</span>
          <span class="r num">{r.new ? "—" : formatUnits(r.current_units)}</span>
          <span class="r num">{formatUnits(r.units)}</span>
          <span class="r num" class:up={d > 0} class:down={d < 0} class:faint={d === 0}>{d === 0 ? "same" : `${d > 0 ? "+" : "−"}${formatUnits(Math.abs(d))}`}</span>
          <span class="r num">{r.price == null ? "—" : formatPlain(r.price)}{#if r.book_cost != null}<span class="faint small"> · book {amountsHidden() ? MASK : formatPlain(r.book_cost)}</span>{/if}</span>
        </div>
      {/each}
    {/if}
    <div class="row">
      <span class="muted small">
        {plan.parsed_lines} of {plan.total_lines} {plan.total_lines === 1 ? "line" : "lines"} parsed
        {#if plan.skipped.length} · skipped: {plan.skipped.map(([n, why]) => `line ${n} (${why})`).join(", ")}{/if}
        {#if plan.rows.length} · {changes} {changes === 1 ? "change" : "changes"}, {prices} {prices === 1 ? "price" : "prices"}, recorded as trades dated <b>{date}</b>{/if}
        {#if plan.untouched.length} · not in paste, left as is: {plan.untouched.join(", ")}{/if}
        {#if error}<span class="bad"> · {error}</span>{/if}
      </span>
      <span class="acts">
        <button type="button" class="link" onclick={onclose}>Cancel</button>
        <button type="button" class="btn" id="apply-paste" onclick={applyPlan} disabled={busy || locked || !plan.mutations.length}>
          {busy ? "Saving…" : plan.mutations.length ? `Apply ${changes ? `${changes} ${changes === 1 ? "change" : "changes"}` : `${prices} ${prices === 1 ? "price" : "prices"}`}` : "Nothing to apply"}
        </button>
      </span>
    </div>
  {:else}
    <div class="row"><span class="faint small">{error ?? "The preview appears as you paste."}</span><button type="button" class="link" onclick={onclose}>Cancel</button></div>
  {/if}
</div>

<style>
  .paste { display: flex; flex-direction: column; gap: 10px; padding: 12px 0 8px 18px; border-top: 1px solid var(--border); }
  .hint { margin: 0; font-size: 12px; }
  textarea { width: 100%; min-height: 96px; background: var(--bg); border: 1px solid var(--border); border-radius: 7px; padding: 8px 10px; color: var(--text); font: inherit; font-family: var(--mono); font-size: 12px; resize: vertical; }
  textarea:focus-visible { outline: 2px solid var(--accent); outline-offset: -1px; border-color: transparent; }
  .prev { display: grid; grid-template-columns: 1fr 100px 100px 110px 150px; gap: 12px; font-size: 13px; padding: 6px 0; border-top: 1px solid var(--border); }
  .prev.h { border-top: 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-3); padding-bottom: 2px; }
  .r { text-align: right; }
  .up { color: var(--positive); }
  .down { color: var(--negative); }
  .new { color: var(--accent); font-size: 11px; margin-left: 6px; }
  .small { font-size: 12px; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .acts { display: flex; gap: 14px; align-items: center; }
  .link { color: var(--accent); background: none; border: 0; font: inherit; cursor: pointer; font-size: 13px; padding: 0; }
  .btn { background: var(--accent); color: var(--bg); border: 0; border-radius: 8px; padding: 8px 14px; font: inherit; font-weight: 600; cursor: pointer; }
  .btn:disabled { opacity: 0.45; cursor: default; }
  .bad { color: var(--negative); }
  code { font-family: var(--mono); font-size: 11px; }
  @media (max-width: 700px) { .prev { grid-template-columns: 1fr 90px 90px; } .prev span:nth-child(2), .prev span:nth-child(5) { display: none; } }
</style>
