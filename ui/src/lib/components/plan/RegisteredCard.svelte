<script lang="ts">
  // One registered account type (TFSA, FHSA or RRSP), all its accounts added up: value, put in vs
  // growth, room, the years of room, and each account. Setup is two answers, previewed before saving.
  import type { AccountView, CraLimits, Mutation, RegisteredView, Standing } from "../../api";
  import { formatDate, formatMoney, parseAmount, todayIso } from "../../format/money";
  import { roomLine } from "../../networth/plan";
  import { accountBars, columns, nextYearAdds, preview, roomStatus, stripLabels, valueBar } from "../../networth/registered";
  import Money from "../amounts/Money.svelte";

  let { r, limits, accounts, currency, year, saving, onsave }: {
    r: RegisteredView;
    limits: CraLimits;
    /** Accounts of this kind, for logging a contribution. */
    accounts: AccountView[];
    currency: string;
    year: number;
    saving: boolean;
    onsave: (ms: Mutation[]) => Promise<boolean>;
  } = $props();

  const money = (v: number | string) => formatMoney(v, currency);
  const yearBased = $derived(r.kind !== "rrsp");
  const bar = $derived(valueBar(r));
  const status = $derived(roomStatus(r, year));
  const cols = $derived(columns(r));
  const labels = $derived(stripLabels(r, year));
  const adds = $derived(nextYearAdds(r));
  // Next January's TFSA limit, from CRA's table (the last known limit if it is not out yet).
  const nextLimit = $derived(Number((limits.tfsa.find(([y]) => y === year + 1) ?? limits.tfsa[limits.tfsa.length - 1])?.[1] ?? 0));
  const bars = $derived(accountBars(r));

  // ---- Setup: two answers ----
  type Mode = Standing["kind"];
  let open = $state<"setup" | "log" | "cra" | null>(null);
  const showSetup = $derived(yearBased && (open === "setup" || r.since == null) && open !== "log");
  let since = $state("");
  let mode = $state<Mode>("maxed");
  let amount = $state("");
  const firstYear = $derived(r.kind === "tfsa" ? limits.tfsa_first_year : limits.fhsa_first_year);
  const yearChoices = $derived(Array.from({ length: year - firstYear + 1 }, (_, i) => year - i));
  $effect(() => { if (!since) since = String(r.since ?? (r.kind === "tfsa" ? firstYear : year)); });
  const standing = $derived<Standing>(mode === "maxed" ? { kind: "maxed" } : { kind: mode, amount: parseAmount(amount) ?? "" });
  const pv = $derived(mode !== "maxed" && !amount.trim() ? null : preview(r.kind, Number(since), standing, year, limits));
  let err = $state<string | null>(null);
  async function saveSetup() {
    if (!pv || pv.error) { err = pv?.error ?? "Enter an amount."; return; }
    err = null;
    if (await onsave([{ op: "setup_registered", kind: r.kind, since: Number(since), standing, as_of: todayIso() }])) open = null;
  }

  // ---- Log a contribution, or (RRSP) the room from the notice of assessment ----
  let cAccount = $state("");
  let cAmount = $state("");
  let cDate = $state(todayIso());
  function startLog() { open = open === "log" ? null : "log"; cAccount = accounts[0]?.id ?? ""; cAmount = ""; cDate = todayIso(); }
  async function logContribution() {
    const a = parseAmount(cAmount);
    if (!cAccount || !a || Number(a) === 0) { err = "Enter an amount (negative for a withdrawal)."; return; }
    err = null;
    if (await onsave([{ op: "record_contribution", contribution: { date: cDate, account: cAccount, amount: a } }])) open = null;
  }
  let craAmount = $state("");
  let craDate = $state(todayIso());
  async function saveCra() {
    const a = parseAmount(craAmount);
    if (a == null || Number(a) < 0) { err = "Room is a number of dollars, 0 or more."; return; }
    err = null;
    if (await onsave([{ op: "set_room", kind: r.kind, room: { as_of: craDate, amount: a } }])) open = null;
  }
  const W = 300, H = 40;
</script>

<div class="k" data-kind={r.kind}>
  <div class="kh"><span>{r.label}</span><span class="c">{r.accounts.length} account{r.accounts.length === 1 ? "" : "s"}{#if r.kind === "fhsa" && r.since}{" · "}opened {r.since}{/if}</span></div>
  <div class="big"><Money value={r.value} {currency} size="l" /></div>
  {#if bar && r.growth != null}
    <div class="sub"><b class:loss={Number(r.growth) < 0}>{Number(r.growth) >= 0 ? "+" : "−"}{money(Math.abs(Number(r.growth)))}{#if r.return_pct != null}{" ("}{Number(r.return_pct) > 0 ? "+" : ""}{r.return_pct}%){/if}</b> {Number(r.growth) >= 0 ? "growth" : "below what you"} {Number(r.growth) >= 0 ? `on ${money(r.put_in!)} put in` : `put in (${money(r.put_in!)})`}</div>
    <div class="vbar" title="put in {money(r.put_in!)} · {Number(r.growth) >= 0 ? 'growth' : 'loss'} {money(Math.abs(Number(r.growth)))}">
      <i style="width:{bar.putIn}%;background:var(--fill)"></i>{#if bar.growth > 0}<i style="width:{bar.growth}%;background:var(--positive)"></i>{/if}{#if bar.loss > 0}<i class="lossbar" style="width:{bar.loss}%"></i>{/if}
    </div>
    <div class="f"><span><b class="sw" style="background:var(--fill)"></b>put in {money(r.put_in!)}</span><span><b class="sw" style="background:{Number(r.growth) >= 0 ? 'var(--positive)' : 'var(--negative)'}"></b>{Number(r.growth) >= 0 ? "growth" : "loss"} {money(Math.abs(Number(r.growth)))}</span></div>
  {:else if yearBased}
    <div class="sub faint">Answer two questions to see what you put in and what grew.</div>
  {:else}
    <div class="sub">room depends on income</div>
  {/if}

  {#if showSetup}
    <form class="setup" onsubmit={(e) => { e.preventDefault(); void saveSetup(); }}>
      <label class="q">{r.kind === "tfsa" ? "When did your TFSA room start?" : "When did you open your first FHSA?"}
        <span class="in">
          <select bind:value={since} aria-label="{r.label} start year">
            {#each yearChoices as y}<option value={String(y)}>{r.kind === "tfsa" && y === firstYear ? `${y} or earlier` : y}</option>{/each}
          </select>
          <span class="hint">{r.kind === "tfsa" ? "the year you turned 18, or moved to Canada if later" : "room starts the year you open one"}</span>
        </span>
      </label>
      <div class="q" role="group" aria-label="Where you stand today">Where do you stand today{r.accounts.length > 1 ? `, across all ${r.accounts.length} ${r.label}s` : ""}?
        <span class="seg">
          <button type="button" class:on={mode === "maxed"} aria-pressed={mode === "maxed"} onclick={() => (mode = "maxed")}>Maxed out</button>
          <button type="button" class:on={mode === "put_in"} aria-pressed={mode === "put_in"} onclick={() => (mode = "put_in")}>Put in $…</button>
          <button type="button" class:on={mode === "left"} aria-pressed={mode === "left"} onclick={() => (mode = "left")}>CRA room $…</button>
        </span>
        {#if mode !== "maxed"}
          <input class="amt" bind:value={amount} inputmode="decimal" placeholder={mode === "put_in" ? "total in, minus withdrawals" : "room CRA My Account shows"} aria-label={mode === "put_in" ? "Total put in" : "Room CRA shows"} />
        {/if}
      </div>
      {#if pv}
        <div class="preview" class:bad={pv.error} role="status">
          {#if pv.error}{pv.error}
          {:else if r.kind === "tfsa"}Room since {pv.since} is <span class="money">{money(pv.lifetime)}</span>. That leaves <span class="money">{money(pv.left)}</span> now, and <span class="money">+{money(nextLimit)}</span> on Jan 1.
          {:else}Room since {pv.since} is <span class="money">{money(pv.lifetime)}</span> of the $40,000 lifetime limit. That leaves <span class="money">{money(pv.left)}</span> for {year}.{/if}
        </div>
      {/if}
      <div class="acts">
        <button class="btn2" disabled={saving}>Save</button>
        {#if r.since != null}<button type="button" class="link" onclick={() => (open = null)}>Cancel</button>{/if}
      </div>
    </form>
  {:else if r.room}
    {@const room = r.room}
    <div class="rl">
      {#if status}<span class="pill {status.tone}">{status.text}</span>{/if}
      <span>
        {#if r.kind === "fhsa"}<span class="money">{money(room.left)}</span> <span class="faint">left in {year}</span>
        {:else if r.lifetime != null}<span class="money">{money(room.left)}</span> <span class="faint">left of</span> <span class="money">{money(r.lifetime)}</span> <span class="faint">since {r.since}</span>
        {:else}<span class="money">{money(room.left)}</span> <span class="faint">left · CRA {formatDate(room.as_of)}{room.estimated ? " · est." : ""}</span>{/if}
      </span>
    </div>
    {#if r.kind === "tfsa" && cols.length}
      {@const slot = W / cols.length}
      <svg viewBox="0 0 {W} {H + 13}" class="strip" role="img" aria-label="TFSA room by year since {r.since}: each column is a year's limit, filled as much as it is used">
        {#each cols as c, i}
          {@const h = c.h * H}
          <g>
            <title>{c.year}: {money(c.limit)} limit{c.estimated ? " (estimated)" : ""}{c.future ? ", from Jan 1" : `, ${money(c.used)} used`}</title>
            {#if c.future}<rect x={i * slot + 0.5} y={H - h + 0.5} width={slot - 3} height={h - 1} rx="1.5" class="ghost" />
            {:else}<rect x={i * slot} y={H - h} width={slot - 2.5} height={h} rx="1.5" class="empty" /><rect x={i * slot} y={H - h * c.fill} width={slot - 2.5} height={h * c.fill} rx="1.5" class="used" />{/if}
          </g>
        {/each}
        {#each labels as y, j}
          {@const i = cols.findIndex((c) => c.year === y)}
          <text x={j === labels.length - 1 ? (i + 1) * slot - 2.5 : i * slot} y={H + 11} text-anchor={j === labels.length - 1 ? "end" : "start"}>{y}</text>
        {/each}
      </svg>
      <div class="f"><span>{roomLine(room)}</span>{#if adds != null}<span>+{money(adds)} on Jan 1</span>{/if}</div>
    {:else if r.kind === "fhsa" && r.years.length}
      <div class="cells" role="img" aria-label="FHSA lifetime limit in five $8,000 years">
        {#each r.years as y}
          {@const fill = Number(y.limit) > 0 ? Math.min(1, Number(y.used) / Number(y.limit)) : 0}
          <div class="cell" class:next={y.future && y.year === year + 1} class:future={y.future} title="{y.year}: {money(y.used)} of {money(y.limit)}">
            <i><b style="width:{fill * 100}%"></b></i><span>{y.year}</span>
          </div>
        {/each}
      </div>
      <div class="f"><span>{#if r.lifetime_left != null}<span class="money">{money(r.lifetime_left)}</span> of $40,000 lifetime left{/if}</span>{#if r.closes}<span>open to {r.closes}</span>{/if}</div>
      <div class="f"><span>{roomLine(room)}</span></div>
    {:else}
      <div class="f"><span>{roomLine(room)}</span></div>
    {/if}
    {#if room.note}<div class="f note">{room.note}</div>{/if}
  {/if}

  {#if !showSetup}
    <div class="acts">
      {#if r.room && accounts.length}<button type="button" class="link" onclick={startLog}>+ Log contribution</button>{/if}
      {#if yearBased}<button type="button" class="link" onclick={() => (open = "setup")}>{r.since == null ? "Set up" : "Change answers"}</button>
      {:else}<button type="button" class="link" onclick={() => { open = open === "cra" ? null : "cra"; craAmount = r.room?.left ?? ""; craDate = todayIso(); }}>{r.room ? "Update from notice" : "Add room from your notice of assessment"}</button>{/if}
    </div>
    {#if open === "log"}
      <form class="inline" onsubmit={(e) => { e.preventDefault(); void logContribution(); }}>
        <select bind:value={cAccount} aria-label="Account">{#each accounts as a}<option value={a.id}>{a.name}</option>{/each}</select>
        <input bind:value={cAmount} placeholder="500 or -2000" aria-label="Amount" />
        <input type="date" bind:value={cDate} aria-label="Date" />
        <button class="btn2" disabled={saving}>Log</button><button type="button" class="link" onclick={() => (open = null)}>Cancel</button>
      </form>
    {/if}
    {#if open === "cra"}
      <form class="inline" onsubmit={(e) => { e.preventDefault(); void saveCra(); }}>
        <input bind:value={craAmount} placeholder="room the notice shows" aria-label="Room" />
        <input type="date" bind:value={craDate} aria-label="As of" />
        <button class="btn2" disabled={saving}>Save</button><button type="button" class="link" onclick={() => (open = null)}>Cancel</button>
      </form>
    {/if}
  {/if}
  {#if err}<p class="error">{err}</p>{/if}

  {#if bars.length}
    <div class="accts" aria-label="{r.label} balances">
      {#each bars as a}
        <div class="ar" title="{a.name}: {money(a.value)}"><span class="an">{a.name}</span><span class="ab"><i style="width:{a.pct}%"></i></span><span class="av money">{money(a.value)}</span></div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .k { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .kh { display: flex; justify-content: space-between; align-items: baseline; font-size: 13px; gap: 8px; }
  .kh .c { color: var(--text-3); font-size: 12px; }
  .big :global(.money) { font-family: var(--display); font-size: 2.1rem; line-height: 1; letter-spacing: -0.01em; font-variant-numeric: lining-nums tabular-nums; }
  .sub { font-size: 12px; color: var(--text-2); }
  .sub b { color: var(--positive); font-weight: 400; font-family: var(--figures); }
  .sub b.loss { color: var(--negative); }
  .vbar { display: flex; height: 8px; border-radius: 3px; overflow: hidden; background: var(--surface-2); gap: 1px; }
  .vbar i { display: block; height: 100%; }
  .lossbar { background: repeating-linear-gradient(-45deg, var(--negative) 0 2px, transparent 2px 5px); box-shadow: inset 0 0 0 1px var(--negative); }
  .f { font-size: 11px; color: var(--text-3); display: flex; justify-content: space-between; gap: 8px; }
  .f.note { margin-top: -2px; }
  .sw { display: inline-block; width: 8px; height: 8px; border-radius: 2px; margin-right: 5px; vertical-align: -0.5px; }
  .rl { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; flex-wrap: wrap; font-size: 13px; margin-top: 8px; padding-top: 10px; border-top: 1px dashed var(--border); }
  .pill { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-2); }
  .pill::before { content: ""; width: 7px; height: 7px; border-radius: 50%; background: var(--warn); }
  .pill.done { color: var(--positive); }
  .pill.done::before { background: var(--positive); }
  .pill.over { color: var(--negative); }
  .pill.over::before { background: var(--negative); }
  .strip { display: block; width: 100%; height: auto; margin-top: 2px; }
  .strip .used { fill: var(--fill); }
  .strip .empty { fill: var(--surface-2); }
  .strip .ghost { fill: none; stroke: var(--text-3); stroke-dasharray: 2 2; }
  .strip text { font-size: 10px; fill: var(--text-3); font-family: var(--figures); }
  .cells { display: grid; grid-template-columns: repeat(5, 1fr); gap: 4px; margin-top: 2px; }
  .cell i { display: block; height: 22px; border-radius: 3px; background: var(--surface-2); overflow: hidden; }
  .cell i b { display: block; height: 100%; background: var(--fill); }
  .cell.next i { background: transparent; outline: 1px dashed var(--text-3); outline-offset: -1px; }
  .cell.future:not(.next) i { opacity: 0.6; }
  .cell span { display: block; font-size: 10px; color: var(--text-3); font-family: var(--figures); margin-top: 3px; }
  .accts { display: flex; flex-direction: column; gap: 3px; margin-top: 4px; padding-top: 10px; border-top: 1px dashed var(--border); }
  .ar { display: grid; grid-template-columns: minmax(0, 128px) 1fr 74px; gap: 8px; align-items: center; font-size: 12px; }
  .an { color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ab { height: 6px; }
  .ab i { display: block; height: 100%; border-radius: 2px; background: var(--fill-soft); }
  .av { text-align: right; font-size: 12px; }
  .acts { display: flex; gap: 14px; align-items: center; }
  .link { background: none; border: 0; color: var(--text-2); font: inherit; font-size: 12px; cursor: pointer; padding: 0; text-decoration: underline; text-decoration-color: var(--border); text-underline-offset: 3px; }
  .link:hover { color: var(--text); }
  .setup { margin-top: 8px; padding: 12px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); display: flex; flex-direction: column; gap: 10px; }
  .q { font-size: 12px; color: var(--text-2); display: flex; flex-direction: column; gap: 5px; }
  .in { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .hint { font-size: 11px; color: var(--text-3); }
  select, input { background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: 6px; padding: 4px 8px; font: inherit; font-size: 12px; min-width: 0; }
  .setup select { font-family: var(--figures); }
  .amt { width: 220px; max-width: 100%; font-family: var(--figures); }
  .seg { display: flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; width: fit-content; max-width: 100%; }
  .seg button { background: none; border: 0; border-left: 1px solid var(--border); color: var(--text-2); font: inherit; font-size: 12px; padding: 4px 10px; cursor: pointer; }
  .seg button:first-child { border-left: 0; }
  .seg button.on { background: var(--accent-soft); color: var(--text); }
  .preview { font-size: 12px; color: var(--text-2); border-top: 1px dashed var(--border); padding-top: 8px; }
  .preview .money { color: var(--text); }
  .preview.bad { color: var(--negative); }
  .btn2 { background: var(--accent); color: var(--bg); border: 0; border-radius: 6px; padding: 5px 12px; font: inherit; font-size: 12px; font-weight: 600; cursor: pointer; }
  .btn2:disabled { opacity: 0.5; }
  .inline { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
  .inline input { width: 120px; }
  .error { color: var(--negative); font-size: 12px; margin: 0; }
</style>
