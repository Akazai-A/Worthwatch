<script lang="ts">
  import { onMount } from "svelte";
  import { createBackup, listBackups, errorMessage, type BackupInfo, type LedgerView } from "../../lib/api";

  let { view }: { view: LedgerView } = $props();
  let list = $state<BackupInfo[] | null>(null);
  let busy = $state(false);
  let err = $state<string | null>(null);
  // Shown with ~ for the home folder, as you'd type it in a terminal.
  const dir = $derived(view.ledger_path.replace(/ledger\.yaml$/, "").replace(/^\/(Users|home)\/[^/]+\//, "~/"));

  onMount(async () => {
    try { list = await listBackups(); } catch (e) { err = String(e); }
  });
  async function now() {
    busy = true;
    try { list = await createBackup(); err = null; } catch (e) { err = errorMessage(e); } finally { busy = false; }
  }
  const size = (b: number) => (b > 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);
  const maxBytes = $derived(Math.max(1, ...(list ?? []).map((b) => b.bytes)));
</script>

<div class="backups">
  <section class="card">
    <div class="hrow"><h2>Backups</h2><button class="btn" type="button" disabled={busy} onclick={now}>{busy ? "Backing up…" : "Back up now"}</button></div>
    <p class="muted">Each backup is one <code>.tar.gz</code> of your ledger, fetched prices and fund holdings. One is made automatically each day the app runs; the newest 30 are kept in <code>{dir}backups/</code>. Copy them somewhere else too (a cloud folder, a USB stick).</p>
    {#if err}<p class="error">{err}</p>{/if}
    {#if list && list.length === 0}<p class="faint">No backups yet.</p>{/if}
    {#if list}
      <div class="rows">
        {#each list as b, i}
          <div class="r">
            <span class="d num">{b.created}</span>
            <span class="bar"><i style="width:{(b.bytes / maxBytes) * 100}%"></i></span>
            <span class="s num faint">{size(b.bytes)}</span>
            <a href="/api/backups/{encodeURIComponent(b.name)}" download={b.name}>Download</a>
            {#if i === 0}<span class="new">latest</span>{:else}<span></span>{/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
  <section class="card">
    <h2 class="label">Restore</h2>
    <p class="muted">Stop the app, then unpack a backup into an empty data folder (or pass <code>--force</code> to replace the current one; it is backed up first):</p>
    <pre>WORTHWATCH_DATA=/path/to/data worthwatch restore worthwatch-2026-11-03-090000.tar.gz</pre>
    <p class="faint small">With Docker: <code>docker compose -f deploy/docker-compose.yml stop worthwatch && docker compose -f deploy/docker-compose.yml run --rm worthwatch restore /data/backups/&lt;file&gt; --force</code></p>
  </section>
</div>

<style>
  .backups { display: flex; flex-direction: column; gap: 20px; }
  h2 { margin: 0; font-size: 18px; font-weight: 500; }
  h2.label { font-size: 11px; margin-bottom: 8px; }
  .hrow { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
  .btn { background: var(--text); color: var(--bg); border: 0; border-radius: 6px; padding: 6px 14px; font: inherit; font-size: 13px; cursor: pointer; }
  .btn:disabled { opacity: 0.5; }
  .rows { margin-top: 12px; }
  .r { display: grid; grid-template-columns: 140px 1fr 60px 80px 50px; gap: 12px; align-items: center; padding: 7px 0; border-top: 1px solid var(--border); font-size: 13px; }
  .r:first-child { border-top: 0; }
  .bar { height: 6px; background: var(--surface-2); border-radius: 3px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--fill); }
  .s { text-align: right; font-size: 12px; }
  .r a { color: var(--text-2); font-size: 12px; }
  .new { font-size: 10px; color: var(--positive); }
  pre { background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 10px 12px; font-family: var(--mono); font-size: 12px; white-space: pre-wrap; overflow-wrap: anywhere; }
  code { font-family: var(--mono); font-size: 12px; }
  .small { font-size: 12px; }
  .error { color: var(--negative); }
  @media (max-width: 760px) { .r { grid-template-columns: 1fr 60px 70px; } .bar, .new { display: none; } }
</style>
