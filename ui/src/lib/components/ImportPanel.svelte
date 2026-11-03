<script lang="ts">
  // Import a bank statement: pick the account, drop or paste the CSV/OFX, check the
  // preview (bank layout, sign, duplicates), then import. Nothing is written before that.
  import { ApiError, importStatement, previewImport, type ImportPreview, type Preset } from "../api";
  import { formatDate, formatMoney } from "../money";
  import { prefs } from "../stores/prefs.svelte";

  let {
    accounts,
    currency,
    labels,
    onDone,
    onClose,
  }: {
    accounts: { id: string; name: string }[];
    currency: string;
    labels: Record<string, string>;
    onDone: (msg: string) => void;
    onClose: () => void;
  } = $props();

  let account = $state("");
  $effect(() => { if (!accounts.some((a) => a.id === account)) account = accounts[0]?.id ?? ""; });
  let text = $state("");
  let fileName = $state<string | null>(null);
  let preset = $state<Preset>("auto");
  let flip = $state(false);
  let preview = $state<ImportPreview | null>(null);
  let err = $state<string | null>(null);
  let busy = $state(false);
  let dragging = $state(false);
  const money = (v: string | number) => (prefs.blur ? "*****" : formatMoney(v, currency, 2));

  let seq = 0;
  $effect(() => {
    const req = { account, text, preset, flip };
    if (!req.account || !req.text.trim()) { preview = null; err = null; return; }
    const mine = ++seq;
    previewImport(req).then(
      (p) => { if (mine === seq) { preview = p; err = null; } },
      (e) => { if (mine === seq) { preview = null; err = e instanceof ApiError ? e.message : String(e); } },
    );
  });

  async function readFile(f: File) {
    fileName = f.name;
    text = await f.text();
  }
  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    const f = e.dataTransfer?.files?.[0];
    if (f) void readFile(f);
  }
  async function run() {
    busy = true;
    try {
      const r = await importStatement({ account, text, preset, flip });
      onDone(r.added ? `Imported ${r.added} transaction${r.added === 1 ? "" : "s"}${r.duplicates ? `; ${r.duplicates} already there` : ""}.` : "Nothing new: every row was already imported.");
    } catch (e) {
      err = e instanceof ApiError ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="card imp" aria-label="Import a statement">
  <div class="hrow"><h2 class="label">Import a statement</h2><button type="button" class="link" onclick={onClose}>Close</button></div>
  <div class="form">
    <label>Account
      <select bind:value={account}>{#each accounts as a}<option value={a.id}>{a.name}</option>{/each}</select>
    </label>
    <label>Bank layout
      <select bind:value={preset}>
        {#each preview?.presets ?? [["auto", "Detect"]] as [id, l]}<option value={id}>{l}</option>{/each}
      </select>
    </label>
  </div>
  <div class="drop" class:on={dragging} role="region" aria-label="Drop a CSV or OFX file here"
    ondragover={(e) => { e.preventDefault(); dragging = true; }} ondragleave={() => (dragging = false)} ondrop={onDrop}>
    {#if fileName}<span>{fileName}</span>{:else}<span>Drop a CSV, OFX or QFX file, or</span>{/if}
    <label class="link pick">choose a file<input type="file" accept=".csv,.ofx,.qfx,.txt" onchange={(e) => { const f = e.currentTarget.files?.[0]; if (f) void readFile(f); }} /></label>
    <span class="faint">or paste below</span>
  </div>
  <textarea bind:value={text} rows="4" placeholder="Paste the export here" aria-label="Statement text" spellcheck="false"></textarea>
  {#if err}<p class="err">{err}</p>{/if}
  {#if preview}
    <div class="sum">
      <span><b>{preview.preset_label}</b> layout</span>
      <span>{preview.rows.length} rows{#if preview.from} · {formatDate(preview.from)} to {formatDate(preview.to!)}{/if}</span>
      <span><span class="money">{money(preview.money_out)}</span> out · <span class="money">{money(preview.money_in)}</span> in</span>
      {#if preview.duplicates}<span class="faint">{preview.duplicates} already imported</span>{/if}
      <label class="flip" title="Use when purchases show as money in"><input type="checkbox" bind:checked={flip} /> Flip signs</label>
    </div>
    {#if preview.sign_guessed}<p class="faint small">Purchases should be money out. If the totals look backwards, flip the signs.</p>{/if}
    <div class="rows">
      {#each preview.rows.slice(0, 8) as r}
        <div class="r" class:dup={r.duplicate}>
          <span class="faint">{formatDate(r.date)}</span><span class="d">{r.description}</span>
          <span class="cc">{labels[r.category] ?? r.category}</span>
          <span class="money a" class:in={Number(r.amount) < 0}>{Number(r.amount) < 0 ? "+" : ""}{money(Math.abs(Number(r.amount)))}</span>
        </div>
      {/each}
      {#if preview.rows.length > 8}<div class="faint small more">and {preview.rows.length - 8} more</div>{/if}
    </div>
    {#if preview.skipped.length}<p class="faint small">Skipped {preview.skipped.length} line{preview.skipped.length === 1 ? "" : "s"}: {preview.skipped.slice(0, 3).map(([l, why]) => `line ${l} (${why})`).join(", ")}</p>{/if}
    <div class="acts"><button type="button" class="btn primary" disabled={busy || preview.new_rows === 0} onclick={run}>Import {preview.new_rows} new</button></div>
  {/if}
</section>

<style>
  .imp { display: flex; flex-direction: column; gap: 10px; }
  .hrow { display: flex; justify-content: space-between; align-items: baseline; }
  h2.label { margin: 0; font-weight: 500; }
  .form { display: flex; gap: 12px; flex-wrap: wrap; }
  .form label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--text-2); }
  select, textarea { background: var(--surface-2); color: var(--text); border: 1px solid var(--border); border-radius: 6px; font: inherit; padding: 6px 8px; }
  textarea { font-family: var(--mono); font-size: 12px; resize: vertical; }
  .drop { border: 1px dashed var(--border); border-radius: 8px; padding: 14px; display: flex; gap: 8px; align-items: center; justify-content: center; flex-wrap: wrap; font-size: 13px; color: var(--text-2); }
  .drop.on { border-color: var(--accent); background: var(--accent-soft); }
  .pick input { display: none; }
  .pick { cursor: pointer; }
  .sum { display: flex; gap: 14px; flex-wrap: wrap; align-items: center; font-size: 13px; }
  .flip { margin-left: auto; display: flex; gap: 6px; align-items: center; color: var(--text-2); }
  .rows { border-top: 1px solid var(--border); }
  .r { display: grid; grid-template-columns: 60px 1fr 110px 90px; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--border); font-size: 12px; align-items: center; }
  .r.dup { opacity: 0.45; }
  .d { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cc { font-size: 11px; color: var(--text-2); }
  .a { text-align: right; }
  .in { color: var(--positive); }
  .more { padding-top: 6px; }
  .small { font-size: 12px; margin: 0; }
  .err { color: var(--negative); margin: 0; font-size: 13px; }
  .acts { display: flex; justify-content: flex-end; }
  .btn { border: 1px solid var(--border); background: var(--surface-2); color: var(--text); border-radius: 6px; padding: 6px 12px; font: inherit; font-size: 13px; cursor: pointer; }
  .btn.primary { background: var(--accent); border-color: var(--accent); color: var(--bg); }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .link { background: none; border: 0; color: var(--accent); font: inherit; font-size: 13px; cursor: pointer; padding: 0; }
  @media (max-width: 760px) { .r { grid-template-columns: 50px 1fr 80px; } .cc { display: none; } }
</style>
