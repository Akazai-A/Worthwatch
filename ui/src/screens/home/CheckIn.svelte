<script lang="ts">
  import { apply, ApiError, GROUPS, GROUP_LABEL, KIND_LABEL, ratesInUse, errorMessage, type Account, type AccountKind, type AccountView, type LedgerView, type Mutation } from "../../lib/api";
  import { balanceDelta, bigSwings, buildCheckin, slugify, summarize, type Entry } from "../../lib/networth/checkin";
  import { formatDate, formatPlain, formatUnits, parseNumber, todayIso, MASK, amountsHidden } from "../../lib/format/money";
  import { view as viewStore } from "../../lib/stores/view.svelte";
  import { route } from "../../lib/stores/route.svelte";
  import Money from "../../lib/components/amounts/Money.svelte";
  import PastePanel from "../../lib/components/check-in/PastePanel.svelte";

  let { view }: { view: LedgerView } = $props();
  let pasting = $state<string | null>(null);
  const STALE_DAYS = 45;

  let date = $state(todayIso());
  let entries = $state<Record<string, Entry>>({});
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let adding = $state(false);
  let draft = $state({ name: "", kind: "chequing" as AccountKind, currency: "", institution: "", symbols: "", fx: "" });
  const draftCurrency = $derived((draft.currency.trim() || view.base_currency).toUpperCase());
  const rates = $derived(ratesInUse(view));
  const fxFetchable = $derived(!view.market.offline && view.market.fx_provider !== "off" && view.base_currency === "CAD");
  const needsFx = $derived(draftCurrency !== view.base_currency && !rates[draftCurrency] && !fxFetchable);
  let draftError = $state<string | null>(null);

  const cur = $derived(view.base_currency);
  const locked = $derived(view.problem != null);
  const byGroup = $derived(GROUPS.map((g) => ({ g, rows: view.accounts.filter((a) => a.group === g) })).filter((x) => x.rows.length));
  const result = $derived(buildCheckin(date, view.accounts, entries, rates, cur));
  const after = $derived(Number(view.totals.net_worth) + result.net_change);
  // A balance that moved more than 20% is flagged before saving; Enter (or Save) again keeps it.
  const swings = $derived(bigSwings(view.accounts, entries));
  let swingsSeen = $state("");
  let flagged = $state(false);
  const swingKey = $derived([...swings.keys()].map((id) => `${id}=${entries[id]?.balance}`).join("|"));
  const holdForSwings = $derived(swings.size > 0 && swingsSeen !== swingKey);

  function entry(id: string): Entry {
    return (entries[id] ??= {});
  }
  function setBalance(id: string, v: string) { entry(id).balance = v; }
  function setUnits(id: string, sym: string, v: string) { (entry(id).units ??= {})[sym] = v; }
  function setPrice(id: string, sym: string, v: string) { (entry(id).prices ??= {})[sym] = v; }
  function deltaOf(a: AccountView) {
    const d = balanceDelta(entries[a.id]?.balance ?? "", a.native_value);
    return d;
  }
  function focusFirst() {
    const el = document.querySelector<HTMLInputElement>(".ci input[type=text]");
    el?.focus();
  }
  // Focus the first field once, when the accounts first arrive; live updates must not move the cursor.
  let focused = false;
  $effect(() => { if (!focused && view.accounts.length) { focused = true; queueMicrotask(focusFirst); } });

  async function save() {
    if (saving || result.mutations.length === 0 || result.invalid.length) return;
    if (holdForSwings) { swingsSeen = swingKey; flagged = true; return; }
    saving = true;
    saveError = null;
    try {
      const v = await apply(result.mutations);
      viewStore.set(v);
      entries = {};
      route.go({ name: "home" });
    } catch (e) {
      saveError = e instanceof ApiError && e.line ? `line ${e.line}: ${e.message}` : errorMessage(e);
    } finally {
      saving = false;
    }
  }

  async function addAccount() {
    draftError = null;
    const name = draft.name.trim();
    if (!name) { draftError = "Give the account a name."; return; }
    const currency = (draft.currency.trim() || cur).toUpperCase();
    const id = slugify(name, view.accounts.map((a) => a.id));
    const symbols = draft.symbols.split(/[\s,]+/).map((s) => s.trim().toUpperCase()).filter(Boolean);
    const account: Account = {
      id, name, kind: draft.kind, currency,
      institution: draft.institution.trim() || null,
      positions: symbols.length ? symbols.map((symbol) => ({ symbol, units: "0" })) : null,
    };
    const ms: Mutation[] = [{ op: "upsert_account", account }];
    if (needsFx) {
      const r = parseNumber(draft.fx);
      if (r == null || Number(r) <= 0) { draftError = `Enter how many ${cur} one ${currency} is worth, so the account can be valued.`; return; }
      ms.push({ op: "set_fx", currency, rate: r });
    }
    saving = true;
    try {
      const v = await apply(ms);
      viewStore.set(v);
      draft = { name: "", kind: "chequing", currency: "", institution: "", symbols: "", fx: "" };
      adding = false;
      queueMicrotask(() => document.getElementById(`in-${id}`)?.focus());
    } catch (e) {
      draftError = errorMessage(e);
    } finally {
      saving = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    // Only Enter in this screen's own fields saves; the ⌘K palette's input must not.
    const t = e.target as HTMLElement | null;
    if (e.key === "Enter" && !adding && t?.tagName === "INPUT" && t.closest(".checkin")) { e.preventDefault(); void save(); }
  }
  const kinds = Object.keys(KIND_LABEL) as AccountKind[];
</script>

<svelte:window onkeydown={onKey} />

<div class="checkin">
  <div class="head">
    <h1>Check-in for <input type="date" id="ci-date" bind:value={date} max={todayIso()} aria-label="Check-in date" /></h1>
    <span class="faint hint">Tab moves down · Enter saves · empty carries forward</span>
  </div>

  {#if locked}
    <p class="locked">The ledger file has an error, so nothing can be saved until it is fixed. See the message at the top.</p>
  {/if}

  <section class="card table">
    {#if view.accounts.length}
      <div class="ci h"><span>Account</span><span class="r">Last value</span><span class="r upd">Updated</span><span class="r">New value</span><span class="r">Change</span></div>
    {/if}
    {#each byGroup as { g, rows }}
      <h2 class="label">{GROUP_LABEL[g]}{#if g === "debt"}<span class="faint norm">&nbsp;· amount owed, positive</span>{/if}</h2>
      {#each rows as a}
        {@const stale = a.days_stale == null || a.days_stale > STALE_DAYS}
        {#if a.positions == null}
          {@const d = deltaOf(a)}
          <div class="ci">
            <span class="name">{a.name}<span class="faint inst">{a.institution ?? ""}{a.currency !== cur ? ` · ${a.currency}` : ""}</span></span>
            <span class="r num muted">{a.as_of ? (amountsHidden() ? MASK : formatPlain(a.native_value)) : "—"}</span>
            <span class="r upd" class:warn={stale}>{a.days_stale == null ? "never" : a.days_stale === 0 ? "today" : `${a.days_stale}d`}</span>
            <input type="text" id="in-{a.id}" inputmode="decimal" placeholder={a.as_of ? (amountsHidden() ? MASK : formatPlain(a.native_value)) : "0.00"} value={entries[a.id]?.balance ?? ""} oninput={(e) => setBalance(a.id, e.currentTarget.value)} class:bad={d === "invalid"} disabled={locked} aria-label="{a.name} new value" />
            <span class="r delta num" class:bad={d === "invalid"}>
              {#if d === "invalid"}not a number{:else if d == null}<span class="faint">carry</span>{:else}<Money value={d} currency={a.currency} signed digits={2} tone={a.liability ? "none" : "delta"} />{/if}
            </span>
          </div>
          {#if flagged && swings.has(a.id)}
            <p class="swing" role="status">{swings.get(a.id)! > 0 ? "+" : ""}{Math.round(swings.get(a.id)! * 100)}% from last time. A digit missing? Fix it, or press Enter again to keep it.</p>
          {/if}
        {:else}
          <div class="ci">
            <span class="name">{a.name}<span class="faint inst">{a.institution ?? ""}{a.currency !== cur ? ` · ${a.currency}` : ""}</span></span>
            <span class="r num muted">{a.missing_price ? "—" : amountsHidden() ? MASK : formatPlain(a.native_value)}</span>
            <span class="r upd" class:warn={stale}>{a.days_stale == null ? "never" : a.days_stale === 0 ? "today" : `${a.days_stale}d`}</span>
            <span class="r faint">from holdings</span>
            <span class="r"><button type="button" class="link" id="paste-{a.id}-open" onclick={() => (pasting = pasting === a.id ? null : a.id)} disabled={locked}>{pasting === a.id ? "Close paste" : "Paste holdings"}</button></span>
          </div>
          {#if pasting === a.id}
            <PastePanel account={a} {date} {locked} onclose={() => (pasting = null)} />
          {/if}
          {#each a.holdings as h}
            <div class="ci pos">
              <span class="name">{h.symbol}<span class="faint inst">units · price in {a.currency}</span></span>
              <span class="r num muted">{formatUnits(h.units)}</span>
              <span class="r upd faint" title={h.price_source === "fetched" ? "fetched price; typing one overrides it for this date" : h.price_source === "manual" ? "typed on a check-in" : ""}>{h.price == null ? "no price" : `${formatPlain(h.price)} ${h.price_date ? formatDate(h.price_date) : ""}`}</span>
              <span class="pair">
                <input type="text" id="in-{a.id}-{h.symbol}-units" inputmode="decimal" placeholder={formatUnits(h.units)} value={entries[a.id]?.units?.[h.symbol] ?? ""} oninput={(e) => setUnits(a.id, h.symbol, e.currentTarget.value)} class:bad={result.invalid.includes(`${a.id}:${h.symbol}:units`)} disabled={locked} aria-label="{h.symbol} units" />
                <input type="text" id="in-{a.id}-{h.symbol}-price" inputmode="decimal" placeholder={h.price == null ? "price" : formatPlain(h.price)} value={entries[a.id]?.prices?.[h.symbol] ?? ""} oninput={(e) => setPrice(a.id, h.symbol, e.currentTarget.value)} class:bad={result.invalid.includes(`${a.id}:${h.symbol}:price`)} disabled={locked} aria-label="{h.symbol} price" />
              </span>
              <span class="r delta faint">{entries[a.id]?.units?.[h.symbol] || entries[a.id]?.prices?.[h.symbol] ? "edited" : "carry"}</span>
            </div>
          {/each}
          {#if a.holdings.length === 0}
            <div class="ci pos"><span class="faint">No holdings yet. Use <em>Paste holdings</em>, or add symbols under <code>positions</code> in the ledger file.</span></div>
          {/if}
        {/if}
      {/each}
    {/each}

    {#if view.accounts.length === 0 && !adding}
      <p class="muted intro">No accounts yet. Add each account you want to track, then type today's balances.</p>
    {/if}

    {#if adding}
      <form class="add" onsubmit={(e) => { e.preventDefault(); void addAccount(); }}>
        <label>Name <input type="text" id="add-name" bind:value={draft.name} placeholder="TD Chequing" /></label>
        <label>Kind <select id="add-kind" bind:value={draft.kind}>{#each kinds as k}<option value={k}>{KIND_LABEL[k]}</option>{/each}</select></label>
        <label>Currency <input type="text" id="add-currency" bind:value={draft.currency} placeholder={cur} maxlength="3" /></label>
        <label>Institution <input type="text" id="add-institution" bind:value={draft.institution} placeholder="optional" /></label>
        {#if needsFx}
          <label>1 {draftCurrency} in {cur} <input type="text" id="add-fx" bind:value={draft.fx} placeholder="e.g. 1.38" inputmode="decimal" /></label>
        {:else if draftCurrency !== cur && !rates[draftCurrency]}
          <span class="faint fxnote">The {draftCurrency}/{cur} rate will be fetched from the Bank of Canada on the next refresh.</span>
        {/if}
        {#if ["tfsa", "rrsp", "fhsa", "resp", "non_registered", "crypto"].includes(draft.kind)}
          <label class="wide">Holdings <input type="text" id="add-symbols" bind:value={draft.symbols} placeholder="symbols, e.g. XEQT.TO VFV.TO (leave empty to track a balance)" /></label>
        {/if}
        <div class="actions">
          <button type="submit" class="btn" disabled={saving}>Add account</button>
          <button type="button" class="link" onclick={() => (adding = false)}>Cancel</button>
          {#if draftError}<span class="bad">{draftError}</span>{/if}
        </div>
      </form>
    {:else}
      <button type="button" class="link addrow" id="add-account" onclick={() => { adding = true; queueMicrotask(() => document.getElementById("add-name")?.focus()); }} disabled={locked}>+ Add account</button>
    {/if}
  </section>

  {#if view.accounts.length}
    <div class="foot">
      <span class="sum muted">
        {summarize(result)}{#if result.mutations.length} · net worth <Money value={result.net_change} currency={cur} signed tone="delta" /> → <Money value={after} currency={cur} />{/if}
        {#if result.invalid.length}<span class="bad"> · fix the highlighted inputs</span>{/if}
        {#if saveError}<span class="bad"> · {saveError}</span>{/if}
      </span>
      <button type="button" class="btn" id="save" onclick={save} disabled={saving || locked || result.mutations.length === 0 || result.invalid.length > 0}>{saving ? "Saving…" : "Save check-in"}</button>
    </div>
  {/if}
</div>

<style>
  .checkin { display: flex; flex-direction: column; gap: 16px; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  h1 { font-size: 22px; font-weight: 600; margin: 0; letter-spacing: -0.01em; display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; }
  input[type=date] { background: var(--surface); border: 1px solid var(--border); color: var(--text); border-radius: 7px; padding: 4px 8px; font: inherit; font-size: 15px; font-family: var(--figures, var(--font)); }
  .hint { font-size: 12px; }
  .locked { margin: 0; color: var(--warn); font-size: 13px; }
  .table { display: flex; flex-direction: column; }
  .ci { display: grid; grid-template-columns: 1fr 110px 80px 170px 110px; gap: 12px; align-items: center; padding: 7px 0; border-top: 1px solid var(--border); }
  .ci.h { border-top: 0; font-size: 12px; color: var(--text-3); padding: 0 0 6px; }
  .ci.pos { padding-left: 18px; font-size: 13px; }
  h2 { margin: 12px 0 4px; font-weight: 500; }
  h2:first-of-type { margin-top: 4px; }
  .norm { text-transform: none; letter-spacing: 0; }
  .r { text-align: right; }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .inst { margin-left: 8px; font-size: 12px; }
  .upd { font-size: 12px; color: var(--text-2); }
  .warn { color: var(--warn); }
  input[type=text], select { width: 100%; background: var(--bg); border: 1px solid var(--border); border-radius: 7px; padding: 6px 9px; color: var(--text); font: inherit; font-family: var(--figures, var(--font)); }
  .ci input[type=text] { text-align: right; }
  input:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: -1px; border-color: transparent; }
  input.bad { border-color: var(--negative); }
  .bad { color: var(--negative); font-size: 12px; }
  .delta { font-size: 13px; }
  .pair { display: flex; gap: 6px; }
  .pair input { min-width: 0; }
  .intro { margin: 4px 0 8px; }
  .addrow { padding: 10px 0 2px; align-self: flex-start; }
  .link { color: var(--accent); background: none; border: 0; font: inherit; cursor: pointer; font-size: 13px; padding: 0; }
  .link:disabled { color: var(--text-3); cursor: default; }
  .add { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 14px; padding: 12px 0 4px; border-top: 1px solid var(--border); margin-top: 8px; }
  .add label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--text-2); }
  .add .wide, .add .actions { grid-column: 1 / -1; }
  .fxnote { font-size: 12px; align-self: end; padding-bottom: 8px; }
  .actions { display: flex; gap: 14px; align-items: center; }
  .btn { background: var(--accent); color: var(--bg); border: 0; border-radius: 8px; padding: 9px 16px; font: inherit; font-weight: 600; cursor: pointer; }
  .btn:disabled { opacity: 0.45; cursor: default; }
  .btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .foot { position: sticky; bottom: 0; background: var(--bg); padding: 14px 0 8px; display: flex; justify-content: space-between; align-items: center; gap: 14px; border-top: 1px solid var(--border); flex-wrap: wrap; }
  .sum { font-size: 13px; }
  .swing { margin: 0 0 6px; padding-left: 8px; border-left: 2px solid var(--accent); color: var(--accent); font-size: 12px; }
  code { font-family: var(--mono); font-size: 12px; }
  @media (max-width: 700px) {
    .ci { grid-template-columns: 1fr 130px; }
    .ci .r:not(.delta), .ci .upd, .ci.h { display: none; }
    .ci .delta { grid-column: 2; text-align: right; }
    .add { grid-template-columns: 1fr; }
    .ci.pos { grid-template-columns: 1fr 180px; }
    .ci.pos .delta { display: none; }
  }
  /* Phone: the bottom tab bar is fixed, so the sticky footer (and Save) sits just above it. */
  @media (max-width: 760px) {
    .foot { bottom: calc(53px + env(safe-area-inset-bottom)); }
  }
</style>
