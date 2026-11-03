<script lang="ts">
  import {
    apply, ApiError, getSettings, makeShortcut, rotateToken,
    SOURCE_LABEL, type Bill, type CategoryRule, type FeedMode, type LedgerView, type Mutation, type SettingsView, type TxnSource,
  } from "../lib/api";
  import { formatDate, formatMoney, parseAmount } from "../lib/money";
  import { budget } from "../lib/stores/budget.svelte";
  import { view as viewStore } from "../lib/stores/view.svelte";

  let { view }: { view: LedgerView } = $props();
  const DOCS = "https://github.com/Akazai-A/Worthwatch/blob/main/docs/apple-pay-feed.md";

  let s = $state<SettingsView | null>(null);
  let err = $state<string | null>(null);
  let note = $state<string | null>(null);
  let busy = $state(false);
  let reveal = $state(false);

  async function loadSettings() {
    try { s = await getSettings(); } catch (e) { err = e instanceof ApiError ? e.message : String(e); }
  }
  $effect(() => { void view.version; void loadSettings(); void budget.ensure(view.version, null); });
  const b = $derived(budget.current);
  const settings = $derived(b?.settings ?? {});
  const feed = $derived(settings.feed ?? { mode: "off" as FeedMode });
  const labels = $derived(Object.fromEntries((b?.options ?? []).map((o) => [o.id, o.label])));

  async function save(ms: Mutation[]) {
    err = null;
    try { viewStore.set(await apply(ms)); return true; } catch (e) { err = e instanceof ApiError ? e.message : String(e); return false; }
  }
  const setMode = (mode: FeedMode) => save([{ op: "set_feed", feed: { ...feed, mode } }]);

  let tsUrl = $state("");
  $effect(() => { tsUrl = feed.tailscale_url ?? s?.tailscale_url ?? ""; });
  async function saveUrl() {
    const u = tsUrl.trim().replace(/\/+$/, "");
    if (u && !/^https?:\/\//.test(u)) { err = "The address starts with https://"; return; }
    if (await save([{ op: "set_feed", feed: { ...feed, tailscale_url: u || null } }])) note = "Saved. Make the Shortcut again so the phone uses it.";
  }

  async function shortcut() {
    busy = true; err = null;
    try { s = await makeShortcut(); note = "Shortcut saved to iCloud Drive. On your iPhone, open Files › iCloud Drive › Shortcuts › Worthwatch and tap it."; }
    catch (e) { err = e instanceof ApiError ? e.message : String(e); }
    finally { busy = false; }
  }
  async function newToken() {
    if (!confirm("Make a new token? The Shortcut on your phone stops working until you make it again.")) return;
    s = await rotateToken();
    note = "New token made. Make the Shortcut again and re-import it on your phone.";
  }
  function copy(text: string) { void navigator.clipboard?.writeText(text); note = "Copied."; }
  const ago = (iso: string | null) => (iso ? new Intl.DateTimeFormat("en-CA", { dateStyle: "medium", timeStyle: "short" }).format(new Date(iso)) : "never");
  const serveCmd = "tailscale serve --bg --set-path /api/ingest http://127.0.0.1:7777/api/ingest";

  // ---- Sources and cards ----
  const setSource = (account: string, source: TxnSource) => save([{ op: "set_txn_source", account, source }]);
  const unlink = (card: string) => save([{ op: "link_card", card, account: null }]);

  // ---- Bills ----
  let bName = $state(""), bAmount = $state(""), bDay = $state(""), bMatch = $state("");
  async function addBill() {
    const amount = parseAmount(bAmount);
    if (!bName.trim() || !amount || Number(amount) <= 0) { err = "A bill needs a name and a positive amount."; return; }
    const day = bDay ? Number(bDay) : null;
    const bill: Bill = { name: bName.trim(), amount, every: "month", category: "bills", matches: bMatch.trim() || null, day: day && day >= 1 && day <= 31 ? day : null };
    if (await save([{ op: "set_bills", bills: [...(settings.bills ?? []), bill] }])) { bName = bAmount = bDay = bMatch = ""; }
  }
  const removeBill = (i: number) => save([{ op: "set_bills", bills: (settings.bills ?? []).filter((_, j) => j !== i) }]);

  // ---- Rules ----
  let rText = $state(""), rCat = $state("dining");
  async function addRule() {
    if (!rText.trim()) { err = "A rule needs some text to look for."; return; }
    const rules: CategoryRule[] = [...(settings.rules ?? []), { contains: rText.trim(), category: rCat }];
    if (await save([{ op: "set_category_rules", rules }])) rText = "";
  }
  const removeRule = (i: number) => save([{ op: "set_category_rules", rules: (settings.rules ?? []).filter((_, j) => j !== i) }]);
</script>

<div class="settings">
  <h1>Settings</h1>
  {#if err}<p class="err" role="alert">{err}</p>{/if}
  {#if note}<p class="note">{note} <button type="button" class="link" onclick={() => (note = null)}>OK</button></p>{/if}

  <section class="card">
    <h2 class="label">Apple Pay taps arrive through</h2>
    <div class="seg" role="radiogroup" aria-label="Apple Pay feed">
      {#each [["off", "Off"], ["icloud", "iCloud Drive"], ["tailscale", "Tailscale"]] as [m, l]}
        <button type="button" role="radio" aria-checked={feed.mode === m} class:on={feed.mode === m} onclick={() => setMode(m as FeedMode)}>{l}</button>
      {/each}
    </div>

    {#if feed.mode === "off"}
      <p class="muted small">Taps are optional. Statements you import are enough for a budget; the feed adds Apple Pay purchases the moment you pay, before the statement has them.</p>
    {:else}
      <div class="flow" aria-label="How a tap gets here">
        <span class="node">Wallet tap</span><span class="arr">→</span>
        <span class="node">Shortcut on your iPhone</span><span class="arr">→</span>
        {#if feed.mode === "icloud"}
          <span class="node">iCloud Drive › Shortcuts › Worthwatch</span><span class="arr">→</span><span class="node">this Mac reads it every 10 s</span>
        {:else}
          <span class="node">your tailnet</span><span class="arr">→</span><span class="node">tailscale serve</span><span class="arr">→</span><span class="node">this server</span>
        {/if}
      </div>

      {#if s}
        <div class="steps">
          {#if feed.mode === "icloud"}
            <span class="ck" class:ok={s.icloud.found}>{s.icloud.found ? "✓" : ""}</span>
            <span>{#if s.icloud.found}Folder found: <code>{s.icloud_dir}</code>{:else}Waiting for the folder <code>{s.icloud_dir ?? "iCloud Drive/Shortcuts/Worthwatch"}</code>. It appears once the Shortcut is saved there or runs the first time. This Mac must be signed in to the same Apple ID.{/if}
              {#if s.icloud.not_downloaded.length}<br /><span class="warn">iCloud hasn't downloaded {s.icloud.not_downloaded.join(", ")} yet. In Finder, right-click the Worthwatch folder and choose Keep Downloaded.</span>{/if}</span>
            <span></span>
          {:else}
            <span class="ck" class:ok={!!s.tailscale_url}>{s.tailscale_url ? "✓" : ""}</span>
            <span>This Mac's Tailscale address{#if s.tailscale_detected} <span class="faint">(found by asking Tailscale)</span>{/if}
              <form class="row" onsubmit={(e) => { e.preventDefault(); void saveUrl(); }}><input bind:value={tsUrl} placeholder="https://mac-mini.tail1234.ts.net" aria-label="Tailscale address" /><button class="btn">Save</button></form></span>
            <span></span>
            <span class="ck"></span>
            <span>Expose only the tap endpoint on your tailnet (the rest of the app stays on this Mac):<br /><code class="cmd">{serveCmd}</code></span>
            <button type="button" class="btn" onclick={() => copy(serveCmd)}>Copy</button>
            <span class="ck ok">✓</span>
            <span>Token <code class="tok">{reveal ? s.token : "•".repeat(12) + s.token.slice(-4)}</code> <span class="faint">is built into your Shortcut; only <code>{s.ingest_path}</code> accepts it.</span></span>
            <span class="btns"><button type="button" class="link" onclick={() => (reveal = !reveal)}>{reveal ? "Hide" : "Show"}</button><button type="button" class="link" onclick={newToken}>New token</button></span>
          {/if}

          <span class="ck" class:ok={s.taps > 0}>{s.taps > 0 ? "✓" : ""}</span>
          <span>{#if s.taps > 0}Last tap received {ago(s.last_tap)} · {s.taps} so far{:else}No taps yet. Make a test purchase once the steps below are done.{/if}</span>
          <span></span>

          <span class="ck" class:ok={!!s.shortcut_file}>{s.shortcut_file ? "✓" : ""}</span>
          <span>Get the Shortcut onto your iPhone.
            {#if s.can_sign}This Mac signs it and saves it to iCloud Drive; open it from Files on the phone.{#if s.shortcut_file} <span class="faint">Saved as <code>{s.shortcut_file}</code></span>{/if}
            {:else}Signing needs a Mac. <a href="/api/settings/shortcut.plist" download>Download the unsigned file</a> and run <code>shortcuts sign --mode anyone -i file.plist -o "Worthwatch Tap.shortcut"</code> on one.{/if}</span>
          {#if s.can_sign}<button type="button" class="btn primary" disabled={busy || (feed.mode === "tailscale" && !s.tailscale_url)} onclick={shortcut}>{s.shortcut_file ? "Make again" : "Make Shortcut"}</button>{:else}<span></span>{/if}

          <span class="ck"></span>
          <span>Turn on the Wallet automation (once, on the phone).
            <details><summary>Steps</summary>
              <ol>
                <li>Open Shortcuts › Automation › <b>+</b> › <b>Transaction</b>.</li>
                <li>Pick the Wallet cards to follow, choose <b>Run Immediately</b>, then <b>Next</b>.</li>
                <li>Tap <b>New Blank Automation</b>. Add a <b>Dictionary</b> action with four text keys: <code>merchant</code> = Merchant, <code>amount</code> = Amount, <code>card</code> = Card or Pass, <code>name</code> = Name.</li>
                <li>Add <b>Run Shortcut</b>, choose <b>Worthwatch Tap</b>, and pass it the Dictionary. Tap Done.</li>
              </ol>
              <a href={DOCS} target="_blank" rel="noreferrer">Full guide with troubleshooting</a>
            </details></span>
          <span></span>
        </div>
        {#if s.icloud.errors.length && feed.mode === "icloud"}<p class="warn small">{s.icloud.errors.slice(0, 3).join(" · ")}</p>{/if}
      {/if}
    {/if}
  </section>

  <section class="card">
    <h2 class="label">Where each account's transactions come from</h2>
    {#if !b}<p class="faint">Loading…</p>
    {:else if b.accounts.length === 0}<p class="muted small">Add chequing, savings or credit card accounts on <a href="#/check-in">Check-in</a>.</p>
    {:else}
      {#each b.accounts as a}
        <div class="acct">
          <span class="gd" style="background:var(--g-{a.group})"></span>
          <span>{a.name}
            <span class="cards">{#if a.cards.length}{#each a.cards as c}<span class="card-chip">Wallet: “{c}” <button type="button" class="x" title="Unlink this card" aria-label="Unlink {c}" onclick={() => unlink(c)}>×</button></span>{/each}{:else}no Wallet card{a.kind === "credit_card" ? " yet: it links itself the first time you tap it" : ""}{/if}</span>
          </span>
          <span class="pills" role="radiogroup" aria-label="Source for {a.name}">
            {#each (a.cards.length ? ["statement", "apple_pay_statement", "apple_pay"] : ["statement"]) as src}
              <button type="button" role="radio" aria-checked={a.source === src} class="pill" class:on={a.source === src} onclick={() => setSource(a.id, src as TxnSource)}>{SOURCE_LABEL[src as TxnSource]}</button>
            {/each}
          </span>
          <span class="faint small">{a.last_statement ? `last statement row ${formatDate(a.last_statement)}` : "no statement imported"}</span>
        </div>
      {/each}
      <p class="faint small">Apple Pay only: taps are the record, at the amount before any tip. Paying a card from chequing counts as a transfer, never as spending twice.</p>
    {/if}
  </section>

  <div class="two">
    <section class="card">
      <h2 class="label">Bills</h2>
      <p class="faint small">Fixed amounts that leave every month. A statement row with the name (or the text you give) and an amount within 10% marks it paid.</p>
      {#each settings.bills ?? [] as bill, i}
        <div class="li"><span>{bill.name}{#if bill.day}<span class="faint"> · day {bill.day}</span>{/if}{#if bill.matches}<span class="faint"> · “{bill.matches}”</span>{/if}</span><span class="money">{formatMoney(bill.amount, view.base_currency)}</span><button type="button" class="x" aria-label="Remove {bill.name}" onclick={() => removeBill(i)}>×</button></div>
      {/each}
      <form class="add" onsubmit={(e) => { e.preventDefault(); void addBill(); }}>
        <input bind:value={bName} placeholder="Rent" aria-label="Bill name" />
        <input bind:value={bAmount} placeholder="1950" aria-label="Amount" class="n" />
        <input bind:value={bDay} placeholder="day" aria-label="Day of month" class="d" />
        <input bind:value={bMatch} placeholder="statement text (optional)" aria-label="Statement text" />
        <button class="btn">Add</button>
      </form>
    </section>

    <section class="card">
      <h2 class="label">Category rules</h2>
      <p class="faint small">Checked before the built-in list of Canadian merchants. First match wins.</p>
      {#each settings.rules ?? [] as r, i}
        <div class="li"><span>contains “{r.contains}”</span><span class="cc">{labels[r.category] ?? r.category}</span><button type="button" class="x" aria-label="Remove rule {r.contains}" onclick={() => removeRule(i)}>×</button></div>
      {/each}
      <form class="add" onsubmit={(e) => { e.preventDefault(); void addRule(); }}>
        <input bind:value={rText} placeholder="COSTCO" aria-label="Text to look for" />
        <select bind:value={rCat} aria-label="Category">{#each b?.options ?? [] as o}<option value={o.id}>{o.label}</option>{/each}</select>
        <button class="btn">Add</button>
      </form>
    </section>
  </div>
</div>

<style>
  .settings { display: flex; flex-direction: column; gap: 20px; }
  h1 { font-size: 20px; font-weight: 500; margin: 0; }
  .card h2 { margin: 0 0 12px; font-weight: 500; }
  .err { color: var(--negative); margin: 0; }
  .note { margin: 0; color: var(--text-2); font-size: 13px; }
  .warn { color: var(--warn); }
  .small { font-size: 12px; }
  code { font-family: var(--mono); font-size: 12px; color: var(--text-2); word-break: break-all; }
  .cmd { display: inline-block; margin-top: 4px; background: var(--surface-2); padding: 3px 6px; border-radius: 4px; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
  .seg button { padding: 7px 14px; font: inherit; font-size: 13px; color: var(--text-2); border: 0; border-right: 1px solid var(--border); background: none; cursor: pointer; }
  .seg button:last-child { border-right: 0; }
  .seg button.on { background: var(--accent-soft); color: var(--text); }
  .flow { display: flex; align-items: center; gap: 10px; margin: 16px 0; font-size: 12px; color: var(--text-2); flex-wrap: wrap; }
  .node { border: 1px solid var(--border); border-radius: 8px; padding: 6px 10px; background: var(--surface-2); color: var(--text); }
  .arr { color: var(--text-3); }
  .steps { display: grid; grid-template-columns: 22px 1fr auto; gap: 12px 12px; font-size: 13px; align-items: start; }
  .ck { width: 16px; height: 16px; border-radius: 50%; border: 1.5px solid var(--text-3); display: grid; place-items: center; font-size: 10px; margin-top: 2px; }
  .ck.ok { background: var(--positive); border-color: var(--positive); color: var(--bg); }
  .row { display: flex; gap: 6px; margin-top: 6px; }
  input, select { background: var(--surface-2); color: var(--text); border: 1px solid var(--border); border-radius: 6px; font: inherit; font-size: 13px; padding: 5px 8px; min-width: 0; }
  .row input { flex: 1; max-width: 340px; }
  details { margin-top: 6px; color: var(--text-2); }
  details ol { margin: 8px 0; padding-left: 20px; display: flex; flex-direction: column; gap: 4px; }
  summary { cursor: pointer; color: var(--accent); }
  .btn { border: 1px solid var(--border); background: var(--surface-2); color: var(--text); border-radius: 6px; padding: 5px 10px; font: inherit; font-size: 12px; cursor: pointer; white-space: nowrap; }
  .btn.primary { background: var(--accent); border-color: var(--accent); color: var(--bg); }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .btns { display: flex; gap: 10px; }
  .link { background: none; border: 0; color: var(--accent); font: inherit; cursor: pointer; padding: 0; font-size: 12px; }
  .acct { display: grid; grid-template-columns: 8px 1fr auto 150px; gap: 12px; align-items: center; padding: 9px 0; border-bottom: 1px solid var(--border); font-size: 13px; }
  .gd { width: 8px; height: 8px; border-radius: 50%; }
  .cards { display: flex; gap: 6px; flex-wrap: wrap; font-size: 12px; color: var(--text-3); margin-top: 2px; }
  .card-chip { display: inline-flex; gap: 4px; align-items: center; }
  .pills { display: flex; gap: 6px; flex-wrap: wrap; justify-content: flex-end; }
  .pill { font: inherit; font-size: 11px; border-radius: 999px; padding: 3px 10px; border: 1px solid var(--border); color: var(--text-2); background: none; cursor: pointer; white-space: nowrap; }
  .pill.on { background: var(--accent-soft); color: var(--text); border-color: transparent; }
  .x { background: none; border: 0; color: var(--text-3); cursor: pointer; font-size: 14px; padding: 0 4px; }
  .x:hover { color: var(--negative); }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; }
  .li { display: grid; grid-template-columns: 1fr auto 20px; gap: 10px; align-items: center; padding: 6px 0; border-bottom: 1px solid var(--border); font-size: 13px; }
  .cc { font-size: 11px; border-radius: 999px; padding: 2px 9px; background: var(--surface-2); color: var(--text-2); }
  .add { display: flex; gap: 6px; margin-top: 10px; flex-wrap: wrap; }
  .add input { flex: 1 1 90px; }
  .add .n { flex: 0 1 70px; } .add .d { flex: 0 1 50px; }
  @media (max-width: 760px) {
    .two { grid-template-columns: 1fr; }
    .acct { grid-template-columns: 8px 1fr; }
    .acct .pills { grid-column: 2; justify-content: flex-start; }
    .acct > .faint { grid-column: 2; }
    .steps { grid-template-columns: 22px 1fr; }
    .steps > :nth-child(3n) { grid-column: 2; justify-self: start; }
  }
</style>
