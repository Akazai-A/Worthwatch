<script lang="ts">
  // The add-a-purchase form: amount, what, category, paid with, date. Used under
  // Budget's Transactions header and under Play's token. State lives in the store.
  import type { Account, CategoryOption } from "../api";
  import { todayIso } from "../money";
  import { payable } from "../spend";
  import { addSpend } from "../stores/addspend.svelte";

  let { accounts, options, onsaved }: { accounts: Account[]; options: CategoryOption[]; onsaved?: () => void } = $props();
  const pays = $derived(payable(accounts));
  const d = $derived(addSpend.draft);
  const today = todayIso();

  async function submit(e: Event) {
    e.preventDefault();
    if (await addSpend.submit()) onsaved?.();
  }
</script>

<form class="add" onsubmit={submit} aria-label="Add a purchase">
  <label class="f amt"><span>Amount</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input id="spend-amount" bind:value={d.amount} inputmode="decimal" placeholder="$0.00" autocomplete="off" autofocus />
  </label>
  <label class="f what"><span>What</span>
    <input id="spend-what" bind:value={d.merchant} placeholder="Farmers market" maxlength="120" autocomplete="off" />
  </label>
  <label class="f"><span>Category</span>
    <select id="spend-category" bind:value={d.category}>
      <option value="">Pick for me</option>
      {#each options.filter((o) => o.id !== "income" && o.id !== "transfer") as o}<option value={o.id}>{o.label}</option>{/each}
    </select>
  </label>
  <label class="f"><span>Paid with</span>
    <select id="spend-account" bind:value={d.account}>
      <option value="">Cash</option>
      {#each pays as a}<option value={a.id}>{a.name}</option>{/each}
    </select>
  </label>
  <label class="f date"><span>Date</span>
    <input id="spend-date" type="date" bind:value={d.date} max={today} />
  </label>
  <div class="go">
    <button type="submit" class="btn primary" disabled={addSpend.busy}>Add</button>
    <button type="button" class="link" onclick={() => addSpend.close()}>Cancel</button>
  </div>
  {#if addSpend.error}<p class="err" role="alert">{addSpend.error}</p>{/if}
  {#if d.account}<p class="hint">On an account you import, this waits for the statement row like a tap does, so it's never counted twice.</p>{/if}
</form>

<style>
  .add { display: grid; grid-template-columns: 96px minmax(120px, 1fr) 140px 150px 136px auto; gap: 8px 10px; align-items: end; padding: 12px; border: 1px dashed var(--accent); border-radius: 10px; background: color-mix(in oklab, var(--accent) 6%, transparent); margin-bottom: 8px; }
  .f { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .f span { font-size: 11px; color: var(--text-3); }
  input, select { height: 30px; background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: 6px; font: inherit; font-size: 13px; padding: 5px 8px; min-width: 0; width: 100%; box-sizing: border-box; }
  .amt input { font-family: var(--figures); text-align: right; }
  input:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: -1px; }
  .go { display: flex; gap: 10px; align-items: center; padding-bottom: 2px; }
  .btn { border: 1px solid var(--accent); background: var(--accent); color: var(--bg); border-radius: 6px; padding: 5px 14px; font: inherit; font-size: 13px; font-weight: 600; cursor: pointer; }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .link { background: none; border: 0; color: var(--text-2); font: inherit; font-size: 12px; cursor: pointer; padding: 0; }
  .err { grid-column: 1 / -1; margin: 0; color: var(--negative); font-size: 12px; }
  .hint { grid-column: 1 / -1; margin: 0; color: var(--text-3); font-size: 12px; }
  @media (max-width: 760px) {
    .add { grid-template-columns: 1fr 1fr; }
    .what { grid-column: 1 / -1; grid-row: 1; }
    .go { grid-column: 1 / -1; }
  }
</style>
