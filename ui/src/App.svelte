<script lang="ts">
  import { onMount } from "svelte";
  import { getView, type LedgerView } from "./lib/api";
  import Money from "./lib/components/Money.svelte";

  let view = $state<LedgerView | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try { view = await getView(); } catch (e) { error = String(e); }
  });

  const groups = ["cash", "investments", "property", "debt"] as const;
  const labels = { cash: "Cash", investments: "Investments", property: "Property", debt: "Debt" };
</script>

<main>
  <header>
    <span class="brand">worthwatch</span>
    <span class="asof">{view ? `as of ${view.as_of}` : ""}</span>
  </header>

  {#if error}
    <p class="error">{error}</p>
  {:else if !view}
    <p class="muted">Loading…</p>
  {:else}
    <section class="hero">
      <div class="label">Net worth</div>
      <Money value={view.totals.net_worth} currency={view.base_currency} size="xl" />
      <div class="sub muted">
        Assets <Money value={view.totals.assets} size="s" /> · Debt <Money value={view.totals.liabilities} size="s" />
      </div>
    </section>

    {#if view.accounts.length === 0}
      <section class="card empty">
        <p>No accounts yet. Add them in <code>data/ledger.yaml</code> (see <code>data/ledger.example.yaml</code>), or wait for the Check-in screen in Stage 1.</p>
      </section>
    {:else}
      {#each groups as g}
        {@const rows = view.accounts.filter((a) => a.group === g)}
        {#if rows.length}
          <section class="card">
            <h2>{labels[g]}</h2>
            {#each rows as a}
              <div class="row">
                <span class="name">{a.name}<span class="muted inst">{a.institution ?? ""}</span></span>
                <span class="right">
                  {#if a.missing_fx || a.missing_price}<span class="warn" title="missing price or FX rate">!</span>{/if}
                  <Money value={a.value} currency={view.base_currency} />
                  <span class="stale" class:old={(a.days_stale ?? 0) > 45}>{a.days_stale == null ? "—" : `${a.days_stale}d`}</span>
                </span>
              </div>
            {/each}
          </section>
        {/if}
      {/each}
    {/if}
  {/if}
</main>

<style>
  main { max-width: 760px; margin: 0 auto; padding: 32px 16px; }
  header { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 40px; }
  .brand { font-weight: 600; letter-spacing: 0.04em; color: var(--text-2); }
  .asof { font-size: 0.8rem; color: var(--text-3); }
  .hero { margin-bottom: 32px; }
  .label { font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-3); margin-bottom: 4px; }
  .sub { margin-top: 6px; }
  .card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 16px; margin-bottom: 12px; }
  .card h2 { font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-3); margin: 4px 0 8px; font-weight: 500; }
  .row { display: flex; justify-content: space-between; padding: 8px 0; border-top: 1px solid var(--border); }
  .row:first-of-type { border-top: 0; }
  .inst { margin-left: 8px; font-size: 0.8rem; }
  .right { display: flex; gap: 12px; align-items: baseline; }
  .stale { font-size: 0.75rem; color: var(--text-3); min-width: 2.5em; text-align: right; }
  .stale.old { color: var(--negative); }
  .warn { color: var(--negative); font-weight: 700; }
  .muted { color: var(--text-2); }
  .error { color: var(--negative); }
  .empty p { margin: 4px 0; color: var(--text-2); }
  code { font-size: 0.85em; }
</style>
