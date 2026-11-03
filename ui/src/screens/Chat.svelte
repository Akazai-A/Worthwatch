<script lang="ts">
  // Chat: type it like a text, get a diff card, approve. The server proposes and
  // checks; approval goes through /api/apply like every other write. Proposals made
  // over MCP (Claude Desktop) wait here as cards of their own.
  import { onMount, tick } from "svelte";
  import { apply, decidePending, ratesInUse, ApiError, type LedgerView, type Plan } from "../lib/api";
  import { EXAMPLES, fmtUnits, providerLine } from "../lib/chat";
  import { chat, type Turn } from "../lib/stores/chat.svelte";
  import { view as viewStore } from "../lib/stores/view.svelte";
  import { formatDate, maskAmounts } from "../lib/money";
  import PlanCard from "../lib/components/PlanCard.svelte";
  import Sparkline from "../lib/charts/Sparkline.svelte";

  let { view, docked = false }: { view: LedgerView; docked?: boolean } = $props();
  const base = $derived(view.base_currency);
  const rates = $derived(ratesInUse(view));
  const locked = $derived(view.problem != null);
  const prov = $derived(providerLine(view.assistant));
  let text = $state("");
  let thread: HTMLDivElement | undefined = $state();
  let input: HTMLInputElement | undefined = $state();
  let pendingError = $state<Record<number, string>>({});
  const series = $derived(view.series.slice(-12).map((p) => Number(p.net_worth)));

  async function scrollDown() {
    await tick();
    thread?.scrollTo({ top: thread.scrollHeight, behavior: "smooth" });
  }
  async function send() {
    const t = text.trim();
    if (!t || chat.busy) return;
    text = "";
    const p = chat.send(t);
    void scrollDown();
    await p;
    void scrollDown();
    input?.focus();
  }
  async function approve(turn: Turn, plan: Plan) {
    try {
      viewStore.set(await apply(plan.mutations));
      const what = plan.rows.length === 1 ? `${plan.rows[0].account_name} → ${plan.rows[0].to == null ? "" : fmtUnits(Number(plan.rows[0].to))}` : `${plan.rows.length} changes`;
      chat.update(turn.id, { plan, status: "approved", outcome: `Saved · ${what}` });
    } catch (e) {
      const msg = e instanceof ApiError && e.line ? `line ${e.line}: ${e.message}` : e instanceof Error ? e.message : String(e);
      chat.update(turn.id, { status: "failed", outcome: `Not saved: ${msg}` });
    }
  }
  async function decide(id: number, action: "approve" | "discard") {
    try {
      viewStore.set(await decidePending(id, action));
    } catch (e) {
      pendingError[id] = e instanceof Error ? e.message : String(e);
    }
  }
  function ago(iso: string): string {
    const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
    if (s < 60) return "just now";
    if (s < 3600) return `${Math.round(s / 60)} min ago`;
    return `${Math.round(s / 3600)} h ago`;
  }
  const isNetWorthAnswer = (a: string) => a.startsWith("Net worth");
  onMount(() => { input?.focus(); void scrollDown(); });
</script>

<div class="chat" class:docked>
  <div class="head">
    {#if !docked}<h1>Chat</h1>{/if}
    <span class="prov" title={view.assistant.url ?? ""}><span class="pd {prov.dot}"></span>{prov.text}</span>
  </div>

  <div class="thread" bind:this={thread}>
    {#if chat.turns.length === 0 && view.pending.length === 0}
      <div class="intro">
        <p class="muted">Type balances and trades the way you would text them. You get a card showing every change before anything is written.</p>
        <div class="ex">
          {#each EXAMPLES as e}<button type="button" class="chip" onclick={() => { text = e; input?.focus(); }}>{e}</button>{/each}
        </div>
        <p class="faint small">Claude Desktop can propose too: add <code>worthwatch mcp</code> as an MCP server and its proposals wait here.</p>
      </div>
    {/if}

    {#each chat.turns as t (t.id)}
      <div class="me">{t.text}</div>
      {#if t.error}
        <div class="card ans"><span class="bad">Could not ask: {t.error}</span></div>
      {:else if !t.reply}
        <div class="card ans faint">Thinking…</div>
      {:else if t.reply.preview}
        {@const pv = t.reply.preview}
        <div class="card send">
          <div><b>Send to Claude (cloud)?</b> <span class="muted">· this exact text leaves your machine, to {new URL(pv.url).host} ({pv.model})</span></div>
          <pre>{pv.prompt.system.split("\nAccounts:\n")[1] ? `Accounts:\n${pv.prompt.system.split("\nAccounts:\n")[1]}` : pv.prompt.system}message: {pv.prompt.user}</pre>
          <details><summary class="faint small">Instructions sent with it</summary><pre>{pv.prompt.system.split("\n\nToday is")[0]}</pre></details>
          <div class="acts">
            <button type="button" class="btn" onclick={() => chat.send(t.text, true, t.id)} disabled={chat.busy}>Send</button>
            <span class="hint">No balances or values are sent: only ids, names, kinds, currencies and symbols.</span>
          </div>
        </div>
      {:else if t.plan}
        {#if t.plan.answer}
          <div class="card ans">
            <span class="answer">{maskAmounts(t.plan.answer)}</span>
            {#if isNetWorthAnswer(t.plan.answer) && series.length > 1}<span class="sp"><Sparkline values={series} width={96} height={22} /></span>{/if}
            <span class="meta">read-only · answered from the ledger, nothing proposed</span>
          </div>
        {/if}
        {#if t.plan.rows.length || t.plan.rejected.length}
          <PlanCard
            plan={t.plan}
            {base}
            {rates}
            now={view.totals.net_worth}
            {locked}
            badge={t.reply.used === "shorthand" ? null : t.reply.used === "cloud" ? "Claude" : t.reply.used}
            approveText={t.plan.rows.length > 1 ? `Approve ${t.plan.rows.length}` : "Approve"}
            status={t.status}
            outcome={t.outcome}
            onapprove={(p) => approve(t, p)}
            oncancel={() => chat.update(t.id, { status: "cancelled", outcome: "Cancelled, nothing written" })}
            onedit={(p) => chat.update(t.id, { plan: p })}
          />
        {:else if !t.plan.answer}
          <div class="card ans muted">Nothing to propose in that. Try “td chq 5102” or “sold 20 XEQT in the TFSA”.</div>
        {/if}
        {#if t.reply.note && !view.assistant.offline}<span class="note faint">{t.reply.note}</span>{/if}
      {/if}
    {/each}

    {#each view.pending as p (p.id)}
      <div class="wide" title={p.text ? `“${p.text}”` : ""}>
        <PlanCard
          plan={p.plan}
          {base}
          {rates}
          now={view.totals.net_worth}
          {locked}
          title={p.origin === "mcp" ? "Claude Desktop" : p.origin}
          badge="MCP"
          meta={ago(p.created)}
          cancelText="Discard"
          onapprove={() => decide(p.id, "approve")}
          oncancel={() => decide(p.id, "discard")}
        />
        {#if pendingError[p.id]}<span class="bad small">{pendingError[p.id]}</span>{/if}
      </div>
    {/each}
  </div>

  <form class="compose" onsubmit={(e) => { e.preventDefault(); void send(); }}>
    <input bind:this={input} bind:value={text} type="text" aria-label="Message" placeholder="Type it like a text: “td chq 5102, visa 640, sold 20 XEQT in the TFSA” · or ask “what is my net worth?”" disabled={chat.busy} />
    <button type="submit" class="btn" disabled={chat.busy || !text.trim()}>{chat.busy ? "…" : "Send"}</button>
  </form>
  <span class="hint">Enter sends · nothing is written until you approve · hover a row to see the words it came from · dated {formatDate(view.as_of)}</span>
</div>

<style>
  .chat { display: flex; flex-direction: column; gap: 12px; height: calc(100vh - 56px); min-height: 420px; }
  .chat.docked { height: auto; flex: 1; min-height: 0; }
  .docked .intro { padding: 8px 0; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  h1 { font-size: 22px; font-weight: 600; margin: 0; letter-spacing: -0.01em; }
  .prov { font-size: 12px; color: var(--text-2); display: inline-flex; gap: 8px; align-items: center; }
  .pd { width: 7px; height: 7px; border-radius: 50%; display: inline-block; background: var(--positive); }
  .pd.cloud { background: var(--accent); }
  .pd.off { background: var(--text-3); }
  .thread { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; padding: 4px 2px 8px; }
  .me { align-self: flex-end; max-width: 72%; background: var(--surface-2); border-radius: 12px 12px 2px 12px; padding: 8px 12px; white-space: pre-wrap; }
  .card { max-width: 92%; }
  .wide :global(.card) { max-width: 100%; border-radius: 12px; }
  .ans { padding: 10px 14px; border-radius: 12px 12px 12px 2px; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 12px; align-self: flex-start; }
  .ans .meta { flex-basis: 100%; font-size: 12px; color: var(--text-3); }
  .sp { color: var(--accent); }
  .send { border-color: var(--text-3); border-radius: 12px 12px 12px 2px; display: flex; flex-direction: column; gap: 8px; }
  pre { margin: 0; font-family: var(--mono); font-size: 12px; background: var(--bg); border: 1px solid var(--border); border-radius: 7px; padding: 8px 10px; color: var(--text-2); white-space: pre-wrap; max-height: 180px; overflow: auto; }
  details pre { margin-top: 6px; }
  .note { font-size: 12px; margin-top: -6px; }
  .intro { display: flex; flex-direction: column; gap: 10px; padding: 24px 0; max-width: 620px; }
  .intro p { margin: 0; }
  .ex { display: flex; gap: 8px; flex-wrap: wrap; }
  .chip { background: var(--surface); border: 1px solid var(--border); color: var(--text-2); border-radius: 16px; padding: 5px 12px; font: inherit; font-size: 13px; cursor: pointer; }
  .chip:hover { color: var(--text); border-color: var(--text-3); }
  .compose { display: flex; gap: 10px; align-items: center; border-top: 1px solid var(--border); padding-top: 12px; }
  .compose input { flex: 1; min-width: 0; background: var(--surface); border: 1px solid var(--border); border-radius: 9px; padding: 10px 12px; color: var(--text); font: inherit; }
  .compose input:focus-visible { outline: 1px solid var(--text-3); outline-offset: -1px; border-color: transparent; }
  .acts { display: flex; gap: 14px; align-items: center; flex-wrap: wrap; }
  .btn { background: var(--text); color: var(--bg); border: 0; border-radius: 8px; padding: 9px 16px; font: inherit; font-weight: 600; cursor: pointer; white-space: nowrap; }
  .btn:disabled { opacity: 0.35; cursor: default; }
  .hint { font-size: 11px; color: var(--text-3); }
  .small { font-size: 12px; }
  .bad { color: var(--negative); }
  code { font-family: var(--mono); font-size: 12px; }
  @media (max-width: 760px) { .chat { height: calc(100vh - 110px); } .me { max-width: 88%; } .card { max-width: 100%; } }
</style>
