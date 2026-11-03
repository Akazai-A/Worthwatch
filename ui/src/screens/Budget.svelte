<script lang="ts">
  import PrivacyToggle from "../lib/components/PrivacyToggle.svelte";
  import { apply, ApiError, setCategory, type LedgerView, type Mutation, type Txn } from "../lib/api";
  import { bullet, byDay, counts, monthLabel, paceLine, shiftMonth, STATUS_LABEL } from "../lib/budget";
  import { formatDate, formatMoney, parseAmount, todayIso } from "../lib/money";
  import { budget } from "../lib/stores/budget.svelte";
  import { prefs } from "../lib/stores/prefs.svelte";
  import { view as viewStore } from "../lib/stores/view.svelte";
  import RunningTotal from "../lib/charts/RunningTotal.svelte";
  import ImportPanel from "../lib/components/ImportPanel.svelte";
  import Sparkline from "../lib/charts/Sparkline.svelte";

  let { view, month, importing = false }: { view: LedgerView; month: string | null; importing?: boolean } = $props();

  $effect(() => { void budget.ensure(view.version, month); });
  const b = $derived(budget.current);
  const cur = $derived(view.base_currency);
  const money = (v: string | number, digits = 0) => (prefs.blur ? "*****" : formatMoney(v, cur, digits));
  const labels = $derived(Object.fromEntries((b?.options ?? []).map((o) => [o.id, o.label])));
  const pace = $derived(b ? paceLine(b) : null);
  const paceFrac = $derived(b ? b.day / b.days : 0);
  const thisMonth = todayIso().slice(0, 7);
  const shownMonth = $derived(b?.month.slice(0, 7) ?? thisMonth);
  const canNext = $derived(shownMonth < thisMonth);
  const canPrev = $derived(b?.first_month != null && b.first_month.slice(0, 7) < shownMonth);
  const cards = $derived(view.accounts.filter((a) => a.kind === "credit_card"));
  const accountName = (id: string | null) => view.accounts.find((a) => a.id === id)?.name ?? "No account";
  const groupOf = (id: string | null) => view.accounts.find((a) => a.id === id)?.group ?? "debt";
  const importable = $derived((b?.accounts ?? []).map((a) => ({ id: a.id, name: a.name })));

  let showImport = $state(false);
  $effect(() => { if (importing) showImport = true; });
  let note = $state<string | null>(null);
  let err = $state<string | null>(null);
  let filter = $state<string | null>(null);
  let showReview = $state(false);
  let showAll = $state(false);
  const LIMIT = 40;

  async function save(ms: Mutation[]) {
    err = null;
    try {
      viewStore.set(await apply(ms));
      return true;
    } catch (e) {
      err = e instanceof ApiError ? e.message : String(e);
      return false;
    }
  }

  // ---- Link a new Wallet card ----
  let linkTo = $state<Record<string, string>>({});
  async function link(card: string) {
    const account = linkTo[card] ?? cards[0]?.id;
    if (account) await save([{ op: "link_card", card, account }]);
  }

  // ---- Limits ----
  let editing = $state<string | null>(null);
  let limitText = $state("");
  function startLimit(cat: string, current: string | null) { editing = cat; limitText = current ?? ""; }
  async function saveLimit() {
    if (!editing) return;
    const amount = limitText.trim() === "" ? null : parseAmount(limitText);
    if (limitText.trim() !== "" && (amount == null || Number(amount) < 0)) { err = "A budget is a dollar amount, 0 or more."; return; }
    if (await save([{ op: "set_budget_limit", category: editing, amount }])) editing = null;
  }

  // ---- Categories on transactions ----
  let remember = $state<{ merchant: string; category: string; account: string | null } | null>(null);
  async function recategorize(t: Txn, category: string) {
    await setCategory(t.id, category);
    if (t.tap_id && t.tap_id !== t.id) await setCategory(t.tap_id, category);
    remember = { merchant: t.merchant, category, account: t.account };
    budget.reload();
    void budget.ensure(view.version, month, true);
  }
  async function rememberRule() {
    if (!remember) return;
    const rules = [...(b?.settings.rules ?? []), { contains: remember.merchant, category: remember.category }];
    if (await save([{ op: "set_category_rules", rules }])) { note = `“${remember.merchant}” is ${labels[remember.category] ?? remember.category} from now on.`; remember = null; }
  }

  const txns = $derived((b?.txns ?? []).filter((t) => filter == null || t.category === filter));
  const days = $derived(byDay(showAll ? txns : txns.slice(0, LIMIT), (t) => counts(t.category)));
  const lastTap = $derived(b?.feed.last_tap ? ago(b.feed.last_tap) : null);
  function ago(iso: string): string {
    const mins = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
    if (mins < 1) return "just now";
    if (mins < 60) return `${mins} min ago`;
    if (mins < 60 * 24) return `${Math.round(mins / 60)} h ago`;
    return `${Math.round(mins / 1440)} days ago`;
  }
  const dayLabel = (iso: string) => new Intl.DateTimeFormat("en-CA", { weekday: "short", month: "short", day: "numeric", timeZone: "UTC" }).format(new Date(iso + "T00:00:00Z"));
  function sub(t: Txn): string {
    if (t.tap_amount && t.tap_currency && t.tap_currency !== t.currency) return `tapped ${formatMoney(t.tap_amount, t.tap_currency, 2)}`;
    if (t.tap_amount) {
      const tip = Number(t.amount) - Number(t.tap_amount);
      return tip > 0 ? `tapped ${money(t.tap_amount, 2)} + ${money(tip, 2)} tip` : `tapped ${money(t.tap_amount, 2)}`;
    }
    if (t.status === "review") return `tapped ${formatDate(t.date)} · no statement row yet`;
    return "";
  }
</script>

<div class="budget">
  <div class="top">
    <div class="month">
      <a class="arrow" class:off={!canPrev} href={canPrev ? `#/budget/${shiftMonth(b!.month, -1)}` : undefined} aria-label="Previous month">‹</a>
      <h1>{b ? monthLabel(b.month) : "Budget"}</h1>
      <a class="arrow" class:off={!canNext} href={canNext && b ? `#/budget/${shiftMonth(b.month, 1)}` : undefined} aria-label="Next month">›</a>
      {#if b && b.day < b.days && b.day > 0}<span class="faint small">day {b.day} of {b.days}</span>{/if}
    </div>
    <div class="right">
      {#if b}
        <a class="feed" href="#/settings" title="Apple Pay feed settings">
          {#if b.feed.mode === "off"}<span class="ld off"></span>Apple Pay feed off
          {:else}<span class="ld"></span>Apple Pay via {b.feed.mode === "icloud" ? "iCloud" : "Tailscale"}{#if lastTap} · last tap {lastTap}{/if}{/if}
        </a>
      {/if}
      <button type="button" class="btn" onclick={() => (showImport = !showImport)}>Import statement…</button>
    </div>
  </div>

  {#if err}<p class="err" role="alert">{err}</p>{/if}
  {#if note}<p class="note">{note} <button type="button" class="link" onclick={() => (note = null)}>OK</button></p>{/if}

  {#if showImport}
    {#if importable.length}
      <ImportPanel accounts={importable} currency={cur} {labels} onClose={() => (showImport = false)}
        onDone={(m) => { note = m; showImport = false; budget.reload(); void budget.ensure(view.version, month, true); }} />
    {:else}
      <section class="card"><p class="muted">Add a chequing, savings or credit card account on <a href="#/check-in">Check-in</a> first; a statement is imported into one.</p></section>
    {/if}
  {/if}

  {#if !b}
    <p class="faint">{budget.error ?? "Loading…"}</p>
  {:else}
    <section class="hero">
      <div>
        <div class="label">Spent in {monthLabel(b.month).split(" ")[0]}</div>
        <div class="big money">{money(b.spent)}</div>
        <div class="sub">
          <PrivacyToggle />
          {#if Number(b.budget) > 0}
            of <b class="money">{money(b.budget)}</b> budget ·
            {#if pace!.left >= 0}<b class="money">{money(pace!.left)}</b> left{#if pace!.perDay != null && b.day < b.days} · <b class="money">{money(pace!.perDay)}</b>/day for {b.days - b.day} days{/if}
            {:else}<b class="money neg">{money(-pace!.left)}</b> over{/if}
            {#if b.day < b.days && b.day > 0} · <span class:pos={pace!.vsPace <= 0} class:neg={pace!.vsPace > 0}>{money(Math.abs(pace!.vsPace))} {pace!.vsPace <= 0 ? "under" : "over"} pace</span>{/if}
          {:else}
            no budget set yet · set one per category below
          {/if}
          {#if Number(b.income) > 0} · <span class="faint">{money(b.income)} came in</span>{/if}
        </div>
      </div>
      {#if Number(b.budget) > 0}
        {@const conf = Math.min(100, ((Number(b.spent) - Number(b.provisional)) / Math.max(Number(b.budget), Number(b.spent))) * 100)}
        {@const prov = Math.min(100 - conf, (Number(b.provisional) / Math.max(Number(b.budget), Number(b.spent))) * 100)}
        <div class="meterbox" title="{money(Number(b.spent) - Number(b.provisional))} on statements · {money(b.provisional)} Apple Pay taps not on a statement yet · pace today {money(b.pace)}">
          <div class="meter" class:over={Number(b.spent) > Number(b.budget)}>
            <i class="c" style="width:{conf}%"></i>{#if prov > 0}<i class="p" style="width:{prov}%"></i>{/if}
            {#if b.day < b.days}<span class="tick" style="left:{(Number(b.pace) / Math.max(Number(b.budget), Number(b.spent))) * 100}%"></span>{/if}
          </div>
          <div class="ml"><span>$0</span>{#if b.day < b.days}<span>pace today {money(b.pace)}</span>{/if}<span>{money(b.budget)}</span></div>
          <div class="legend"><span><i class="sw c"></i>On a statement</span>{#if prov > 0}<span><i class="sw p"></i>Apple Pay, not on a statement yet</span>{/if}</div>
        </div>
      {/if}
    </section>

    {#if b.unlinked.length || b.review.length || b.missing_fx || b.store_errors.length}
      <div class="chips">
        {#each b.unlinked as u}
          <div class="chip warn">
            <span class="st review" aria-hidden="true"></span>
            <span><b>{u.card}</b> tapped {u.taps} time{u.taps === 1 ? "" : "s"}. Which account is it?</span>
            {#if cards.length}
              <select aria-label="Account for {u.card}" value={linkTo[u.card] ?? cards[0].id} onchange={(e) => (linkTo = { ...linkTo, [u.card]: e.currentTarget.value })}>
                {#each cards as a}<option value={a.id}>{a.name}</option>{/each}
              </select>
              <button type="button" class="btn" onclick={() => link(u.card)}>Link</button>
            {:else}
              <a href="#/check-in">Add it as a credit card account</a>
            {/if}
          </div>
        {/each}
        {#if b.review.length}
          <button type="button" class="chip" onclick={() => (showReview = !showReview)}>
            <span class="st review" aria-hidden="true"></span>{b.review.length} tap{b.review.length === 1 ? "" : "s"} not on a statement after 7 days <span class="link">{showReview ? "Hide" : "Review"}</span>
          </button>
        {/if}
        {#if b.missing_fx}<div class="chip"><span class="faint">{b.missing_fx} transaction{b.missing_fx === 1 ? "" : "s"} in another currency left out: no exchange rate</span></div>{/if}
        {#if b.store_errors.length}<div class="chip" title={b.store_errors.join("\n")}><span class="faint">{b.store_errors.length} unreadable line{b.store_errors.length === 1 ? "" : "s"} in transactions/</span></div>{/if}
      </div>
      {#if showReview}
        <section class="card">
          <p class="muted small">Each of these was tapped but no statement row matched it within a week. It may have been declined or refunded, or the card's statement isn't imported yet.</p>
          {#each b.review as t}
            <div class="tx"><span class="st review"></span><span class="m">{t.merchant}<small>{formatDate(t.date)} · {t.card}</small></span><span></span><span class="acct">{accountName(t.account)}</span><span class="a money">{money(t.amount, 2)}</span></div>
          {/each}
        </section>
      {/if}
    {/if}

    {#if b.txns.length === 0}
      <section class="card empty">
        <h2>No transactions in {monthLabel(b.month)}</h2>
        <p class="muted">Import a statement from your bank, or turn on the Apple Pay feed in <a href="#/settings">Settings</a> so taps show up as you pay.</p>
        <button type="button" class="btn primary" onclick={() => (showImport = true)}>Import a statement</button>
      </section>
    {:else}
      <section class="card">
        <div class="hrow"><h2 class="label">Running total</h2><span class="faint small">hover a day</span></div>
        <RunningTotal v={b} />
        <div class="legend">
          <span><i class="ln"></i>On a statement</span>
          {#if Number(b.provisional) > 0}<span><i class="ln dash"></i>Including Apple Pay taps</span>{/if}
          {#if Number(b.budget) > 0}<span><i class="ln pace"></i>Budget pace (bills on their day)</span>{/if}
          <span><i class="sw fill"></i>Each day (own scale)</span>
        </div>
      </section>
    {/if}

    <section class="card">
      <div class="hrow"><h2 class="label">By category</h2><span class="faint small">bar is the budget · tick is today's pace · line is the last 6 months</span></div>
      <div class="cats">
        {#each b.categories as c}
          {@const w = bullet(c, paceFrac)}
          <div class="cat">
            <button type="button" class="n" class:on={filter === c.category} onclick={() => (filter = filter === c.category ? null : c.category)} title="Show only {c.label}">{c.label}</button>
            <span class="bar" title="{money(c.spent)} spent{Number(c.provisional) > 0 ? ` (${money(c.provisional)} not on a statement yet)` : ""}{c.limit ? ` of ${money(c.limit)}` : ""}">
              <i class="c" style="width:{w.confirmed}%"></i><i class="p" style="width:{w.provisional}%"></i><i class="o" style="width:{w.over}%"></i>
              {#if w.limitAt != null && w.isOver}<span class="lim" style="left:{w.limitAt}%"></span>{/if}
              {#if w.paceAt != null && b.day < b.days}<span class="pace" style="left:{w.paceAt}%"></span>{/if}
            </span>
            <span class="amt money" class:neg={w.isOver}>{money(c.spent)}
              {#if editing === c.category}
                <form class="lim-f" onsubmit={(e) => { e.preventDefault(); void saveLimit(); }}>
                  <!-- svelte-ignore a11y_autofocus -->
                  <input bind:value={limitText} aria-label="Monthly budget for {c.label}" placeholder="none" autofocus onblur={() => void saveLimit()} />
                </form>
              {:else}
                <button type="button" class="of" onclick={() => startLimit(c.category, c.limit)} title="Set the monthly budget">/ {c.limit ? money(c.limit) : "set"}</button>
              {/if}
            </span>
            <Sparkline values={[...c.history.map(Number), Number(c.spent)]} width={54} height={18} />
          </div>
        {/each}
        {#if b.categories.length === 0}<p class="faint small">Spending shows here by category once there are transactions.</p>{/if}
      </div>
      {#if b.bills.length}
        <div class="bills"><span class="faint">Bills:</span>
          {#each b.bills as bill}
            <span class="bill" title={bill.paid_on ? `paid ${money(bill.paid_amount ?? bill.amount, 2)} on ${formatDate(bill.paid_on)}` : "no matching statement row this month yet"}>
              <span class="bd" class:paid={bill.paid_on}></span>{bill.name} <span class="money">{money(bill.amount)}</span>
              <span class="faint">· {bill.paid_on ? `paid ${formatDate(bill.paid_on)}` : bill.due_day ? `due ${monthLabel(b.month).split(" ")[0].slice(0, 3)} ${bill.due_day}` : "not paid yet"}</span>
            </span>
          {/each}
          <a class="small" href="#/settings">Edit bills</a>
        </div>
      {/if}
    </section>

    {#if b.txns.length}
      <section class="card">
        <div class="hrow">
          <h2 class="label">Transactions</h2>
          {#if filter}<button type="button" class="link small" onclick={() => (filter = null)}>Showing {labels[filter] ?? filter} · show all</button>{/if}
        </div>
        {#if remember}
          <p class="note small">Always put “{remember.merchant}” in {labels[remember.category] ?? remember.category}? <button type="button" class="link" onclick={rememberRule}>Yes, make it a rule</button> <button type="button" class="link faint" onclick={() => (remember = null)}>No</button></p>
        {/if}
        {#each days as d}
          <div class="day"><span>{dayLabel(d.date)}</span><span class="money">{d.total ? money(d.total) : ""}</span></div>
          {#each d.txns as t (t.id)}
            <div class="tx" class:xfer={!counts(t.category)}>
              <span class="st {t.status}" title={STATUS_LABEL[t.status]} aria-label={STATUS_LABEL[t.status]}></span>
              <span class="m" title={t.description ?? ""}>{t.merchant}{#if sub(t)}<small>{sub(t)}</small>{/if}</span>
              <select class="cc" value={t.category} aria-label="Category for {t.merchant}" onchange={(e) => recategorize(t, e.currentTarget.value)}>
                {#each b.options as o}<option value={o.id}>{o.label}</option>{/each}
              </select>
              <span class="acct"><span class="gd" style="background:var(--g-{groupOf(t.account)})"></span>{t.account ? accountName(t.account) : t.card}</span>
              <span class="a money" class:in={Number(t.amount) < 0}>{Number(t.amount) < 0 ? "+" : ""}{t.currency !== cur ? formatMoney(Math.abs(Number(t.amount)), t.currency, 2) : money(Math.abs(Number(t.amount)), 2)}</span>
            </div>
          {/each}
        {/each}
        {#if !showAll && txns.length > LIMIT}<button type="button" class="link more" onclick={() => (showAll = true)}>Show all {txns.length} transactions</button>{/if}
        <div class="legend tl">
          <span><span class="st provisional"></span>Apple Pay tap, waiting for statement</span>
          <span><span class="st confirmed"></span>Tap matched to statement</span>
          <span><span class="st posted"></span>From statement</span>
          <span><span class="st review"></span>Needs a look</span>
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .budget { display: flex; flex-direction: column; gap: 20px; }
  .top { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .month { display: flex; gap: 10px; align-items: center; }
  h1 { font-size: 16px; font-weight: 500; margin: 0; min-width: 130px; text-align: center; }
  .arrow { color: var(--text-2); border: 1px solid var(--border); border-radius: 6px; padding: 0 8px; text-decoration: none; line-height: 22px; }
  .arrow.off { opacity: 0.3; pointer-events: none; }
  .right { display: flex; gap: 14px; align-items: center; flex-wrap: wrap; }
  .feed { display: flex; gap: 6px; align-items: center; font-size: 12px; color: var(--text-3); text-decoration: none; }
  .feed:hover { color: var(--text-2); }
  .ld { width: 7px; height: 7px; border-radius: 50%; background: var(--positive); }
  .ld.off { background: var(--text-3); }
  .small { font-size: 12px; }
  .err { color: var(--negative); margin: 0; }
  .note { margin: 0; color: var(--text-2); font-size: 13px; }
  .btn { border: 1px solid var(--border); background: var(--surface); color: var(--text); border-radius: 6px; padding: 5px 10px; font: inherit; font-size: 12px; cursor: pointer; }
  .btn.primary { background: var(--accent); border-color: var(--accent); color: var(--bg); }
  .link { background: none; border: 0; color: var(--accent); font: inherit; cursor: pointer; padding: 0; }
  .hero { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; align-items: end; }
  .big { font-family: var(--display); font-size: 4rem; font-weight: 500; letter-spacing: -0.01em; line-height: 1; font-variant-numeric: lining-nums tabular-nums; }
  .sub { color: var(--text-2); font-size: 13px; }
  .sub b { font-weight: 500; color: var(--text); }
  .neg { color: var(--negative) !important; }
  .pos { color: var(--positive); }
  .meter { height: 10px; background: var(--surface-2); border-radius: 5px; position: relative; display: flex; gap: 2px; }
  .meter i { display: block; height: 100%; border-radius: 4px; }
  .meter.over { box-shadow: inset 0 0 0 1px var(--negative); }
  .c { background: var(--fill); }
  .p { background: repeating-linear-gradient(135deg, var(--accent) 0 3px, transparent 3px 6px); box-shadow: inset 0 0 0 1px var(--accent); }
  .o { background: var(--negative); }
  .tick { position: absolute; top: -4px; bottom: -4px; width: 2px; background: var(--text-2); }
  .ml { display: flex; justify-content: space-between; font-size: 11px; color: var(--text-3); margin-top: 6px; }
  .legend { display: flex; gap: 16px; font-size: 11px; color: var(--text-2); margin-top: 8px; flex-wrap: wrap; align-items: center; }
  .legend span { display: flex; align-items: center; gap: 6px; }
  .sw { width: 10px; height: 8px; border-radius: 2px; display: inline-block; }
  .sw.fill { background: var(--fill); width: 6px; }
  .ln { width: 14px; height: 0; border-top: 2px solid var(--accent); display: inline-block; }
  .ln.dash { border-top-style: dashed; }
  .ln.pace { border-top: 1px dashed var(--text-3); }
  .chips { display: flex; gap: 10px; flex-wrap: wrap; }
  .chip { display: flex; align-items: center; gap: 10px; border: 1px solid var(--border); border-radius: 8px; padding: 8px 12px; background: var(--surface); font: inherit; font-size: 13px; color: var(--text); }
  button.chip { cursor: pointer; }
  .chip.warn { border-color: color-mix(in oklab, var(--warn) 45%, var(--border)); }
  .chip select, .cc { background: var(--surface-2); color: var(--text); border: 1px solid var(--border); border-radius: 6px; font: inherit; font-size: 12px; padding: 3px 6px; }
  .card h2 { margin: 0 0 12px; font-weight: 500; }
  .empty h2 { font-size: 15px; }
  .hrow { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .cats { display: grid; grid-template-columns: 1fr 1fr; gap: 0 28px; }
  .cat { display: grid; grid-template-columns: 100px 1fr 120px 54px; gap: 10px; align-items: center; padding: 7px 0; border-bottom: 1px solid var(--border); font-size: 13px; }
  .cat .n { background: none; border: 0; color: var(--text); font: inherit; text-align: left; padding: 0; cursor: pointer; }
  .cat .n.on { color: var(--accent); }
  .bar { height: 8px; background: var(--surface-2); border-radius: 4px; position: relative; display: flex; gap: 2px; }
  .bar i { height: 100%; border-radius: 4px; display: block; }
  .bar i[style="width:0%"] { display: none; }
  .bar .pace, .bar .lim { position: absolute; top: -3px; bottom: -3px; width: 2px; background: var(--text-3); }
  .bar .lim { background: var(--text); }
  .amt { text-align: right; font-size: 12px; display: flex; gap: 4px; justify-content: flex-end; align-items: center; }
  .of { background: none; border: 0; color: var(--text-3); font: inherit; cursor: pointer; padding: 0; }
  .of:hover { color: var(--accent); }
  .lim-f input { width: 64px; background: var(--surface-2); border: 1px solid var(--accent); color: var(--text); border-radius: 4px; font: inherit; font-size: 12px; padding: 1px 4px; }
  .bills { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; font-size: 12px; margin-top: 12px; color: var(--text-2); }
  .bill { display: flex; gap: 6px; align-items: center; border: 1px solid var(--border); border-radius: 999px; padding: 3px 10px; }
  .bd { width: 8px; height: 8px; border-radius: 50%; border: 1.5px solid var(--text-3); }
  .bd.paid { background: var(--positive); border-color: var(--positive); }
  .day { font-size: 12px; color: var(--text-3); padding: 12px 0 4px; display: flex; justify-content: space-between; }
  .tx { display: grid; grid-template-columns: 14px 1fr 124px 140px 96px; gap: 12px; align-items: center; padding: 6px 0; border-bottom: 1px solid var(--border); font-size: 13px; }
  .tx.xfer { color: var(--text-3); }
  .m { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m small { color: var(--text-3); margin-left: 6px; font-size: 11px; }
  .cc { justify-self: start; border-radius: 999px; color: var(--text-2); max-width: 124px; }
  .acct { font-size: 12px; color: var(--text-3); display: flex; gap: 6px; align-items: center; overflow: hidden; white-space: nowrap; }
  .gd { width: 6px; height: 6px; border-radius: 50%; flex: none; }
  .a { text-align: right; }
  .in { color: var(--positive); }
  .st { width: 9px; height: 9px; border-radius: 50%; display: inline-block; flex: none; }
  .st.confirmed { background: var(--text-2); }
  .st.posted { background: var(--fill-soft); }
  .st.provisional, .st.unlinked { border: 1.5px solid var(--accent); }
  .st.tap_only { background: var(--accent); }
  .st.review { background: var(--warn); box-shadow: 0 0 0 2px color-mix(in oklab, var(--warn) 30%, transparent); }
  .tl { margin-top: 12px; }
  .more { margin-top: 10px; font-size: 13px; }
  @media (max-width: 760px) {
    .hero { grid-template-columns: 1fr; }
    .big { font-size: 2.6rem; }
    .cats { grid-template-columns: 1fr; }
    .cat { grid-template-columns: 90px 1fr 110px; }
    .cat :global(.spark) { display: none; }
    .tx { grid-template-columns: 12px 1fr 84px; }
    .tx .cc, .tx .acct { display: none; }
  }
</style>
