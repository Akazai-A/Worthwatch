<script lang="ts">
  import PrivacyToggle from "../lib/components/shell/PrivacyToggle.svelte";
  import { apply, ApiError, EVERY_LABEL, type Every, type LedgerView, type Mutation, type Recurring, type RoomKind } from "../lib/api";
  import { formatCompact, formatDate, formatMoney, parseAmount, todayIso } from "../lib/format/money";
  import { milestoneSplit, monthYear, payoffLine, roomBar, roomLine, yearsBetween } from "../lib/networth/plan";
  import { undo } from "../lib/stores/undo.svelte";
  import { plan } from "../lib/stores/plan.svelte";
  import { prefs } from "../lib/stores/prefs.svelte";
  import { view as viewStore } from "../lib/stores/view.svelte";
  import FanChart from "../lib/charts/FanChart.svelte";
  import Money from "../lib/components/amounts/Money.svelte";

  let { view }: { view: LedgerView } = $props();
  const cur = $derived(view.base_currency);

  $effect(() => { void plan.ensure(view.version); });
  // Re-run when an input changes too (ensure is keyed on them).
  $effect(() => { void plan.years; void plan.returnPct; void plan.extra; void plan.target; void plan.ensure(view.version); });

  const p = $derived(plan.current);
  const P = $derived(p?.projection);
  const colMax = $derived(Math.max(1, ...(P?.milestones ?? []).map((m) => Number(m.net_worth))));
  const ledgerReturn = $derived(p ? Number(p.settings.return_pct) : 5);
  const ret = $derived(plan.returnPct != null ? Number(plan.returnPct) : ledgerReturn);
  const tgt = $derived(plan.target != null ? Number(plan.target) : p?.settings.target != null ? Number(p.settings.target) : null);
  const extraN = $derived(Number(plan.extra));
  const changed = $derived(p != null && ((plan.returnPct != null && Number(plan.returnPct) !== ledgerReturn) || (plan.target != null && Number(plan.target) !== Number(p.settings.target ?? 0))));
  // The headline follows the 10y / 20y / 30y toggle.
  // While a slider or the finish line moves: the old projection as a faint ghost, and one line on what changed.
  let ghost = $state<{ points: NonNullable<typeof P>["points"]; date: string | null } | null>(null);
  const hold = () => { if (P && !ghost) ghost = { points: P.points, date: P.target_date ?? null }; };
  const release = () => { setTimeout(() => (ghost = null), 1200); };
  const shift = $derived.by(() => {
    if (!ghost || !P?.target_date || !ghost.date || ghost.date === P.target_date) return null;
    const m = (d: string) => Number(d.slice(0, 4)) * 12 + Number(d.slice(5, 7));
    const months = m(ghost.date) - m(P.target_date);
    return months === 0 ? null : { months, date: P.target_date };
  });
  const headline = $derived(P ? (P.milestones.find((m) => m.years === plan.years) ?? P.milestones[P.milestones.length - 1]) : null);
  const money = (v: number | string) => (prefs.blur ? "*****" : formatMoney(v, cur));
  const compact = (v: number | string) => (prefs.blur ? "*****" : formatCompact(Number(v)));
  const COLH = 180;
  const maxDebt = $derived(P ? Math.max(1, ...P.debts.map((d) => Number(d.balance))) : 1);
  const groupOf = (id: string) => view.accounts.find((a) => a.id === id)?.group ?? "cash";

  let saving = $state(false);
  let err = $state<string | null>(null);
  async function save(ms: Mutation[]) {
    saving = true;
    err = null;
    try {
      viewStore.set(await apply(ms));
      return true;
    } catch (e) {
      err = e instanceof ApiError ? e.message : String(e);
      return false;
    } finally {
      saving = false;
    }
  }

  async function saveDefaults() {
    if (!p) return;
    const ok = await save([{ op: "set_projection", projection: { ...p.settings, return_pct: String(ret), target: tgt != null && tgt > 0 ? String(tgt) : null } }]);
    if (ok) { plan.setReturn(null); plan.setTarget(null); }
  }

  // ---- Recurring rules ----
  let adding = $state(false);
  let rAccount = $state("");
  let rAmount = $state("");
  let rEvery = $state<Every>("month");
  let rNote = $state("");
  const rulesFor = (id: string): Recurring[] => (p?.recurring ?? []).filter((r) => r.account === id).map(({ account, amount, every, note }) => ({ account, amount, every, note }));
  async function addRule() {
    const amount = parseAmount(rAmount);
    if (!rAccount || !amount || Number(amount) <= 0) { err = "Pick an account and a positive amount."; return; }
    const rules = [...rulesFor(rAccount), { account: rAccount, amount, every: rEvery, note: rNote.trim() || null }];
    if (await save([{ op: "set_recurring", account: rAccount, rules }])) { adding = false; rAmount = ""; rNote = ""; }
  }
  async function removeRule(i: number) {
    const r = p!.recurring[i];
    const same = p!.recurring.filter((x) => x.account === r.account);
    const plain = (xs: typeof same) => xs.map(({ account, amount, every, note }) => ({ account, amount, every, note }));
    const keep = plain(same.filter((x) => x !== r));
    if (await save([{ op: "set_recurring", account: r.account, rules: keep }]) !== false) undo.offer(`Removed the ${r.name} rule`, [{ op: "set_recurring", account: r.account, rules: plain(same) }]);
  }

  // ---- Room ----
  const ROOM_KINDS: RoomKind[] = ["tfsa", "rrsp", "fhsa"];
  const ROOM_LABEL: Record<RoomKind, string> = { tfsa: "TFSA", rrsp: "RRSP", fhsa: "FHSA" };
  const missingRoom = $derived(ROOM_KINDS.filter((k) => view.accounts.some((a) => a.kind === k) && !p?.room.some((r) => r.kind === k)));
  let editing = $state<RoomKind | null>(null);
  let eAmount = $state("");
  let eDate = $state(todayIso());
  function startEdit(k: RoomKind) {
    editing = k;
    const r = p?.room.find((x) => x.kind === k);
    eAmount = r ? r.left : "";
    eDate = todayIso();
  }
  async function saveRoom() {
    const amount = parseAmount(eAmount);
    if (!editing || amount == null || Number(amount) < 0) { err = "Room is a number of dollars, 0 or more."; return; }
    if (await save([{ op: "set_room", kind: editing, room: { as_of: eDate, amount } }])) editing = null;
  }
  let logging = $state<RoomKind | null>(null);
  let cAccount = $state("");
  let cAmount = $state("");
  let cDate = $state(todayIso());
  function startLog(k: RoomKind) {
    logging = k;
    cAccount = view.accounts.find((a) => a.kind === k)?.id ?? "";
    cAmount = "";
    cDate = todayIso();
  }
  async function logContribution() {
    const amount = parseAmount(cAmount);
    if (!cAccount || !amount || Number(amount) === 0) { err = "Enter an amount (negative for a withdrawal)."; return; }
    if (await save([{ op: "record_contribution", contribution: { date: cDate, account: cAccount, amount } }])) logging = null;
  }
</script>

<div class="plan">
  <div class="top faint">
    <span>as of {formatDate(view.as_of, "long")} · {cur}</span>
    {#if P}<span>investments {ret}% · cash {p!.settings.cash_pct}% · property {p!.settings.property_pct}%{#if Object.keys(p!.settings.rates ?? {}).length} · {Object.keys(p!.settings.rates ?? {}).length} account rate{Object.keys(p!.settings.rates ?? {}).length > 1 ? "s" : ""}{/if}</span>{/if}
  </div>

  {#if view.accounts.length === 0}
    <section class="card"><h2>Nothing to project yet</h2><p class="muted">Add your accounts on <a href="#/check-in">Check-in</a> first; the plan starts from today's balances.</p></section>
  {:else if !P}
    <p class="faint">{plan.error ?? "Working it out…"}</p>
  {:else}
    {#if headline}
      <section class="hero">
        <h1 class="label">In {headline.years} year{headline.years > 1 ? "s" : ""}, at {ret}%</h1>
        <Money value={headline.net_worth} currency={cur} size="xl" />
        <div class="sub">
          <PrivacyToggle />
          range <b class="money">{compact(headline.low)}–{compact(headline.high)}</b> at {ret - 2}–{ret + 2}%
          {#if tgt != null && tgt > 0}
            · <b class="money">{compact(tgt)}</b> target
            {#if P.target_date}in <b>{yearsBetween(P.start, P.target_date)} yrs</b> ({monthYear(P.target_date)}){:else}not reached within {plan.years} years{/if}
          {/if}
        </div>
      </section>
    {/if}

    <section class="card knobs">
      <label class="knob"><span class="row"><span>Return on investments</span><b>{ret}%</b></span>
        <input type="range" min="0" max="10" step="0.5" value={ret} list="ret-stops" onpointerdown={hold} onchange={release} oninput={(e) => plan.setReturn(e.currentTarget.value)} aria-label="Return on investments, percent a year" />
        <datalist id="ret-stops"><option value="4"></option><option value="6"></option><option value="8"></option></datalist>
        <span class="stops" aria-hidden="true"><span style="left:40%">4%</span><span style="left:60%">6%</span><span style="left:80%">8%</span></span>
        <span class="hint">ledger says {ledgerReturn}% · band ±2 pts</span></label>
      <label class="knob"><span class="row"><span>Save extra each month</span><b class="money">+{money(extraN)}</b></span>
        <input type="range" min="0" max="3000" step="50" value={extraN} onpointerdown={hold} onchange={release} oninput={(e) => plan.setExtra(e.currentTarget.value)} aria-label="Extra saving per month" />
        <span class="hint">on top of {money(P.monthly_saving)}/mo in rules</span></label>
      <label class="knob"><span class="row"><span>Target net worth</span><b class="money">{tgt ? money(tgt) : "none"}</b></span>
        <input type="range" min="0" max="5000000" step="50000" value={tgt ?? 0} onpointerdown={hold} onchange={release} oninput={(e) => plan.setTarget(e.currentTarget.value)} aria-label="Target net worth" />
        <span class="hint">{P.target_date ? `reached ${monthYear(P.target_date)}` : tgt ? "not within the horizon" : "drag to set one"}</span></label>
      <div class="foot">
        <div class="seg" role="group" aria-label="Horizon">
          {#each [10, 20, 30] as y}<button type="button" class:on={plan.years === y} onclick={() => plan.setYears(y)}>{y}y</button>{/each}
        </div>
        {#if shift && tgt}<span class="shift" role="status">{compact(tgt)} moves to {monthYear(shift.date)}, {Math.abs(shift.months)} month{Math.abs(shift.months) === 1 ? "" : "s"} {shift.months > 0 ? "sooner" : "later"}</span>{/if}
        <span class="spacer"></span>
        {#if changed || extraN > 0}<button type="button" class="link" onclick={() => plan.reset()}>Reset</button>{/if}
        {#if changed}<button type="button" class="btn" disabled={saving} onclick={saveDefaults}>Save {ret}%{tgt ? ` and ${compact(tgt)}` : ""} as my defaults</button>{/if}
      </div>
    </section>

    <section class="card">
      <FanChart history={view.series} points={P.points} start={P.start} startNetWorth={P.start_net_worth} milestones={P.milestones} debts={P.debts} target={tgt && tgt > 0 ? tgt : null} currency={cur} blur={prefs.blur}
        ghost={ghost?.points ?? null} ontarget={(v) => plan.setTarget(String(v))} ondrag={(on) => (on ? hold() : release())} />
      <div class="legend">
        <span><i class="sw past"></i>Net worth so far</span>
        <span><i class="sw band"></i>Projected, {ret - 2}–{ret + 2}% band</span>
        <span><i class="sw floor"></i>Today's net worth + money you put in</span>
        {#if tgt}<span><i class="sw target"></i>Target</span>{/if}
      </div>
      {#if P.skipped.length}<p class="warn small">Left out (no FX rate): {P.skipped.join(", ")}</p>{/if}
    </section>

    <!-- One column per milestone, stacked: today's net worth, money put in, growth. Compounding shows as the brass taking over. -->
    <section class="cols" aria-label="What each milestone is made of">
      {#each P.milestones as m, i}
        {@const sp = milestoneSplit(P.start_net_worth, m)}
        {@const h = (Number(m.net_worth) / colMax) * COLH}
        <div class="col" style="width:{100 / P.milestones.length}%" title="today {money(sp.today)} · put in {money(sp.putIn)} · growth and paydown {money(sp.growth)}">
          <div class="cv"><span class="money">{compact(m.net_worth)}</span><span class="cg">{Math.round((sp.growth / sp.total) * 100)}% growth</span></div>
          <div class="stack" style="height:{Math.max(4, h)}px">
            <i class="grow" style="flex:{sp.growth}"></i>
            <i style="flex:{sp.putIn};background:var(--fill-soft)"></i>
            <i style="flex:{sp.today};background:var(--fill)"></i>
          </div>
          <div class="cl">{m.years} year{m.years > 1 ? "s" : ""} · {monthYear(m.date)}</div>
          <div class="cr faint money">{compact(m.low)} – {compact(m.high)}</div>
        </div>
      {/each}
    </section>
    <div class="legend under"><span><i style="background:var(--fill)"></i>Today's net worth</span><span><i style="background:var(--fill-soft)"></i>Money put in</span><span><i class="grow"></i>Growth and debt paydown</span></div>

    <div class="two">
      <section class="card">
        <h2 class="label">Debt payoff</h2>
        {#if P.debts.length === 0}<p class="faint small">No debt. Nothing to pay off.</p>{/if}
        {#each P.debts as d}
          {@const ys = [Number(d.balance), ...d.yearly.map(Number)]}
          {@const pts = ys.map((v, i) => `${(i / Math.max(1, ys.length - 1)) * 240},${34 - (v / maxDebt) * 34}`).join("L")}
          <div class="debt">
            <div class="n">{d.name}<span class="money">{compact(d.balance)} @ {d.rate_pct}%</span></div>
            <div>
              <svg width="100%" height="38" viewBox="0 0 240 38" preserveAspectRatio="none" aria-hidden="true">
                <path d="M0,34L{pts}L240,34Z" class="debt-a" /><path d="M{pts}" class="debt-l" />
              </svg>
              <div class="f"><span>{payoffLine(d, plan.years)}</span><span class="money">{Number(d.interest) > 0 ? `${money(d.interest)} interest` : ""}</span></div>
            </div>
          </div>
        {/each}
        <p class="faint small">Payments come from rules below; set a debt's rate under <code>settings.projection.rates</code>.</p>
      </section>

      <section class="card">
        <h2 class="label">Contribution room {view.as_of.slice(0, 4)}</h2>
        {#each p!.room as r}
          {@const b = roomBar(r)}
          <div class="room">
            <div class="h"><span>{r.label}</span><span><b class="money" class:neg={r.over}>{money(r.left)}</b> <span class="faint">left of {compact(b.room)}</span></span></div>
            <div class="bar" title="used {money(r.used)} · planned to Dec 31 {money(r.planned)} · left {money(r.left)}">
              <i style="width:{b.used}%;background:var(--fill)"></i><i style="width:{b.planned}%;background:var(--fill-soft)"></i>{#if b.over}<i style="width:{b.over}%;background:var(--negative)"></i>{/if}
            </div>
            <div class="f"><span class:neg={r.over}>{roomLine(r)}</span><span>CRA {formatDate(r.as_of)}{r.estimated ? " · est." : ""}</span></div>
            {#if r.note}<div class="f note">{r.note}</div>{/if}
            <div class="acts">
              <button type="button" class="link" onclick={() => startLog(r.kind)}>+ Log contribution</button>
              <button type="button" class="link" onclick={() => startEdit(r.kind)}>Update from CRA</button>
            </div>
            {#if logging === r.kind}
              <form class="inline" onsubmit={(e) => { e.preventDefault(); void logContribution(); }}>
                <select bind:value={cAccount} aria-label="Account">{#each view.accounts.filter((a) => a.kind === r.kind) as a}<option value={a.id}>{a.name}</option>{/each}</select>
                <input bind:value={cAmount} placeholder="500 or -2000" aria-label="Amount" />
                <input type="date" bind:value={cDate} aria-label="Date" />
                <button class="btn" disabled={saving}>Log</button><button type="button" class="link" onclick={() => (logging = null)}>Cancel</button>
              </form>
            {/if}
            {#if editing === r.kind}
              <form class="inline" onsubmit={(e) => { e.preventDefault(); void saveRoom(); }}>
                <input bind:value={eAmount} placeholder="room CRA shows" aria-label="Room" />
                <input type="date" bind:value={eDate} aria-label="As of" />
                <button class="btn" disabled={saving}>Save</button><button type="button" class="link" onclick={() => (editing = null)}>Cancel</button>
              </form>
            {/if}
          </div>
        {/each}
        {#each missingRoom as k}
          <div class="room">
            <div class="h"><span>{ROOM_LABEL[k]}</span><button type="button" class="link" onclick={() => startEdit(k)}>Add room from CRA My Account</button></div>
            {#if editing === k}
              <form class="inline" onsubmit={(e) => { e.preventDefault(); void saveRoom(); }}>
                <input bind:value={eAmount} placeholder="room CRA shows" aria-label="Room" />
                <input type="date" bind:value={eDate} aria-label="As of" />
                <button class="btn" disabled={saving}>Save</button><button type="button" class="link" onclick={() => (editing = null)}>Cancel</button>
              </form>
            {/if}
          </div>
        {/each}
        {#if p!.room.length}<div class="legend"><span><i style="background:var(--fill)"></i>Used</span><span><i style="background:var(--fill-soft)"></i>Planned to Dec 31</span><span><i style="background:var(--surface-2);outline:1px solid var(--border)"></i>Left</span></div>{/if}
      </section>
    </div>

    <section class="card rules">
      <div class="hrow"><h2 class="label">On autopilot</h2><button type="button" class="link" onclick={() => { adding = !adding; rAccount ||= view.accounts[0]?.id ?? ""; }}>{adding ? "Cancel" : "+ Add rule"}</button></div>
      {#if p!.recurring.length === 0 && !adding}<p class="faint small">No recurring rules. Add the deposits and payments that happen on their own, like "$500 to the TFSA every month".</p>{/if}
      {#each p!.recurring as r, i}
        <div class="r">
          <span class="gd" style="background:var(--g-{groupOf(r.account)})"></span>
          <span>{r.name}{#if r.note}<span class="faint">{"\u00a0· "}{r.note}</span>{/if}</span>
          <span class="a"><span class="money">{money(r.amount)}</span> <span class="faint">/ {EVERY_LABEL[r.every]}</span></span>
          <span class="m money">{r.monthly != null ? `${compact(r.monthly)}/mo` : "no FX"}</span>
          <button type="button" class="x" title="Remove this rule" aria-label="Remove rule for {r.name}" onclick={() => removeRule(i)}>×</button>
        </div>
      {/each}
      {#if adding}
        <form class="inline" onsubmit={(e) => { e.preventDefault(); void addRule(); }}>
          <select bind:value={rAccount} aria-label="Account">{#each view.accounts as a}<option value={a.id}>{a.name}</option>{/each}</select>
          <input bind:value={rAmount} placeholder="500" aria-label="Amount" />
          <select bind:value={rEvery} aria-label="How often">{#each Object.entries(EVERY_LABEL) as [k, l]}<option value={k}>every {l}</option>{/each}</select>
          <input bind:value={rNote} placeholder="note (optional)" aria-label="Note" />
          <button class="btn" disabled={saving}>Add</button>
        </form>
      {/if}
    </section>
    {#if err}<p class="error">{err}</p>{/if}
  {/if}
</div>

<style>
  .plan { display: flex; flex-direction: column; gap: 20px; }
  .top { display: flex; justify-content: space-between; gap: 12px; font-size: 12px; flex-wrap: wrap; }
  .sub { font-size: 13px; color: var(--text-2); margin-top: 4px; }
  .sub b { font-family: var(--figures); font-weight: 400; color: var(--text); }
  .knobs { display: grid; grid-template-columns: repeat(3, 1fr); gap: 14px 18px; }
  .knob { display: flex; flex-direction: column; }
  .knob .row { display: flex; justify-content: space-between; font-size: 12px; color: var(--text-2); }
  .knob .row b { font-family: var(--figures); font-weight: 400; color: var(--text); }
  .knob input { width: 100%; accent-color: var(--accent); margin: 8px 0 2px; }
  .hint { font-size: 11px; color: var(--text-3); }
  .foot { grid-column: 1 / -1; display: flex; align-items: center; gap: 12px; }
  .spacer { flex: 1; }
  /* Growth is brass hatch: solid brass stays for the projection line. */
  .grow, .legend i.grow { background: repeating-linear-gradient(-45deg, var(--accent) 0 2px, transparent 2px 5px); box-shadow: inset 0 0 0 1px var(--accent); }
  .stops { position: relative; display: block; height: 14px; margin: -2px 0 2px; font-family: var(--figures); font-size: 10px; color: var(--text-3); }
  .stops span { position: absolute; transform: translateX(-50%); }
  .shift { font-size: 12px; color: var(--accent); }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
  .seg button { background: none; border: 0; color: var(--text-2); font: inherit; font-size: 12px; padding: 3px 10px; cursor: pointer; }
  .seg button.on { background: var(--surface-2); color: var(--text); }
  .btn { background: var(--text); color: var(--bg); border: 0; border-radius: 6px; padding: 5px 12px; font: inherit; font-size: 12px; cursor: pointer; }
  .btn:disabled { opacity: 0.5; }
  .link { background: none; border: 0; color: var(--text-2); font: inherit; font-size: 12px; cursor: pointer; padding: 0; text-decoration: underline; text-decoration-color: var(--border); text-underline-offset: 3px; }
  .link:hover { color: var(--text); }
  .legend { display: flex; gap: 16px; font-size: 11px; color: var(--text-2); margin-top: 8px; flex-wrap: wrap; }
  .legend.under { margin-top: 0; }
  .legend i { display: inline-block; width: 10px; height: 10px; border-radius: 2px; margin-right: 6px; vertical-align: -1px; }
  .sw.past { background: var(--accent); height: 2px; vertical-align: 3px; }
  .sw.band { background: var(--accent-soft); outline: 1px dashed var(--accent); }
  .sw.floor { background: var(--fill-soft); }
  .sw.target { border-top: 1px dotted var(--text-2); height: 0; background: none; vertical-align: 3px; }
  .cols { display: flex; align-items: flex-end; border-top: 1px solid var(--border); padding-top: 16px; }
  .col { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 0 12px; }
  .cv { display: flex; flex-direction: column; align-items: center; }
  .cv .money { font-family: var(--display); font-size: 1.4rem; line-height: 1.1; font-variant-numeric: lining-nums; }
  .cg { font-size: 12px; color: var(--accent); }
  .stack { width: min(120px, 100%); display: flex; flex-direction: column; gap: 1px; }
  .stack i { display: block; min-height: 0; }
  .cl { font-size: 12px; color: var(--text-2); text-align: center; }
  .cr { font-size: 12px; }
  .bar i { display: block; height: 100%; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; align-items: start; }
  h2.label { margin: 0 0 12px; font-weight: 500; }
  .debt { display: grid; grid-template-columns: 110px 1fr; gap: 10px; align-items: center; padding: 6px 0; }
  .debt .n { font-size: 13px; }
  .debt .n span { display: block; font-size: 11px; color: var(--text-3); }
  .debt-a { fill: var(--g-debt); opacity: 0.35; }
  .debt-l { fill: none; stroke: var(--g-debt); stroke-width: 1.5; vector-effect: non-scaling-stroke; }
  .f { font-size: 11px; color: var(--text-3); display: flex; justify-content: space-between; gap: 8px; }
  .f.note { margin-top: 2px; }
  .room { padding: 8px 0 10px; border-top: 1px solid var(--border); }
  .room:first-of-type { border-top: 0; padding-top: 0; }
  .room .h { display: flex; justify-content: space-between; font-size: 13px; }
  .room .h b { font-family: var(--figures); font-weight: 400; }
  .bar { display: flex; height: 10px; border-radius: 3px; overflow: hidden; background: var(--surface-2); margin: 6px 0 4px; }
  .acts { display: flex; gap: 14px; margin-top: 6px; }
  .neg { color: var(--negative) !important; }
  .inline { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 8px; align-items: center; }
  .inline input, .inline select { background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: 6px; padding: 4px 8px; font: inherit; font-size: 12px; min-width: 0; width: 120px; }
  .inline select { width: auto; }
  .hrow { display: flex; justify-content: space-between; align-items: baseline; }
  .rules .r { display: grid; grid-template-columns: 10px 1fr auto 80px 18px; gap: 10px; align-items: center; font-size: 13px; padding: 6px 0; border-top: 1px solid var(--border); }
  .rules .r:first-of-type { border-top: 0; }
  .gd { width: 8px; height: 8px; border-radius: 50%; }
  .a { font-family: var(--figures); white-space: nowrap; }
  .m { color: var(--text-3); font-size: 12px; text-align: right; }
  .x { background: none; border: 0; color: var(--text-3); cursor: pointer; font-size: 15px; padding: 0; }
  .x:hover { color: var(--negative); }
  .small { font-size: 12px; }
  .warn { color: var(--warn); }
  .error { color: var(--negative); }
  code { font-family: var(--mono); font-size: 11px; }
  @media (max-width: 760px) {
    .knobs { grid-template-columns: 1fr; }
    .col { padding: 0 4px; }
    .cv .money { font-size: 1.1rem; }
    .two { grid-template-columns: 1fr; }
    .rules .r { grid-template-columns: 10px 1fr auto 18px; }
    .rules .m { display: none; }
  }
</style>
