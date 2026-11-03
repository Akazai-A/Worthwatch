<script lang="ts">
  // ⌘K / Ctrl+K: jump to an account or a screen, or run an action. Items are built
  // from the current view; matching and ranking live in lib/palette.ts.
  import { tick } from "svelte";
  import { createBackup, refresh, type LedgerView } from "../api";
  import { formatMoney } from "../money";
  import { rank, runs, type Item } from "../palette";
  import { prefs } from "../stores/prefs.svelte";
  import { view as viewStore } from "../stores/view.svelte";

  let { view, open = $bindable(false) }: { view: LedgerView | null; open?: boolean } = $props();
  let q = $state("");
  let sel = $state(0);
  let input = $state<HTMLInputElement>();
  let note = $state<string | null>(null);

  const go = (hash: string) => () => { location.hash = hash; };
  const items = $derived.by((): Item[] => {
    const acc: Item[] = (view?.accounts ?? []).map((a) => ({
      id: `a:${a.id}`, section: "Accounts", label: a.name, hint: `${a.institution ?? ""} ${a.kind}`,
      run: go(`#/accounts/${encodeURIComponent(a.id)}`),
    }));
    const screens: [string, string, string][] = [
      ["Home", "#/", "net worth"],
      ["Check-in", "#/check-in", "enter this month's balances"],
      ["Portfolio", "#/portfolio", "positions"],
      ["Portfolio › Look-through", "#/portfolio/look-through", "companies inside your funds"],
      ["Portfolio › Exposure", "#/portfolio/exposure", "sectors regions"],
      ["Portfolio › Overlap", "#/portfolio/overlap", "funds"],
      ["Changes", "#/changes", "what moved"],
      ["Plan", "#/plan", "projections contribution room"],
      ["Budget", "#/budget", "spending this month apple pay"],
      ["Budget › Import a statement", "#/budget/import", "csv ofx bank export"],
      ["Chat", "#/chat", "update by text"],
      ["Settings", "#/settings", "apple pay shortcut icloud tailscale"],
      ["Wall view", "#/wall", "full screen, for a spare tablet"],
      ["Backups", "#/backups", "download restore"],
    ];
    const nav: Item[] = screens.map(([label, hash, hint]) => ({ id: `g:${hash}`, section: "Go to", label, hint, run: go(hash) }));
    const act: Item[] = [
      { id: "x:blur", section: "Actions", label: prefs.blur ? "Show amounts" : "Hide amounts", hint: "privacy hide", run: () => prefs.toggleBlur() },
      { id: "x:theme", section: "Actions", label: "Cycle theme", hint: "dark light", run: () => prefs.cycleTheme() },
      { id: "x:backup", section: "Actions", label: "Back up now", hint: "backup", run: async () => { const b = await createBackup(); note = `Saved ${b[0]?.name ?? "a backup"}`; location.hash = "#/backups"; } },
      { id: "x:refresh", section: "Actions", label: "Refresh prices", hint: "quotes fx", run: async () => viewStore.set(await refresh()) },
    ];
    return [...acc, ...nav, ...act];
  });
  const hits = $derived(rank(items, q));
  const value = (id: string) => view?.accounts.find((a) => `a:${a.id}` === id);

  $effect(() => {
    if (open) { q = ""; sel = 0; note = null; void tick().then(() => input?.focus()); }
  });
  $effect(() => { void q; sel = 0; });

  function choose(i: number) {
    const h = hits[i];
    if (!h) return;
    open = false;
    void h.item.run();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") { e.preventDefault(); sel = Math.min(hits.length - 1, sel + 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); sel = Math.max(0, sel - 1); }
    else if (e.key === "Enter") { e.preventDefault(); choose(sel); }
    else if (e.key === "Escape") { e.preventDefault(); open = false; }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={() => (open = false)}></div>
  <div class="pal" role="dialog" aria-label="Command palette" aria-modal="true">
    <div class="q">
      <input bind:this={input} bind:value={q} onkeydown={onKey} placeholder="An account, a screen, or an action" aria-label="Search" role="combobox" aria-expanded="true" aria-controls="pal-list" aria-activedescendant="pal-{sel}" />
      <kbd>esc</kbd>
    </div>
    <div class="list" id="pal-list" role="listbox">
      {#each hits as h, i (h.item.id)}
        {#if i === 0 || hits[i - 1].item.section !== h.item.section}<div class="sec">{h.item.section}</div>{/if}
        {@const a = value(h.item.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="r" class:on={i === sel} id="pal-{i}" role="option" aria-selected={i === sel} tabindex="-1" onmouseenter={() => (sel = i)} onclick={() => choose(i)}>
          <span class="i" style={a ? `background:var(--g-${a.group})` : ""}></span>
          <span>{#each runs(h.item.label, h.marks) as r}{#if r.on}<em>{r.text}</em>{:else}{r.text}{/if}{/each}{#if h.item.section === "Go to" && h.item.hint}<span class="faint">{"\u00a0· "}{h.item.hint}</span>{/if}</span>
          <span class="m">{#if a}<span class="money">{a.missing_fx || a.missing_price ? "–" : formatMoney(a.liability ? a.value : a.value, view?.base_currency)}</span>{:else if h.item.id === "x:blur"}⌘⇧H{/if}</span>
        </div>
      {:else}
        <p class="faint none">Nothing matches “{q}”.</p>
      {/each}
    </div>
    <div class="foot"><span>↑↓ move</span><span>↵ open</span><span>⌘K anywhere</span>{#if note}<span class="ok">{note}</span>{/if}</div>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; background: rgba(10, 9, 8, 0.55); backdrop-filter: blur(2px); z-index: 50; }
  .pal { position: fixed; left: 50%; top: 12vh; transform: translateX(-50%); width: min(560px, calc(100vw - 32px)); background: var(--surface); border: 1px solid var(--border); border-radius: 12px; box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45); overflow: hidden; z-index: 51; }
  .q { display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-bottom: 1px solid var(--border); }
  .q input { flex: 1; background: none; border: 0; outline: 0; color: var(--text); font: inherit; font-size: 16px; caret-color: var(--accent); }
  kbd { font-family: var(--mono); font-size: 10px; border: 1px solid var(--border); border-radius: 4px; padding: 1px 5px; color: var(--text-3); }
  .list { max-height: min(60vh, 460px); overflow-y: auto; padding-bottom: 4px; }
  .sec { font-size: 12px; color: var(--text-3); padding: 10px 16px 4px; }
  .r { display: grid; grid-template-columns: 16px 1fr auto; gap: 10px; align-items: center; padding: 8px 16px; font-size: 14px; color: var(--text-2); cursor: pointer; }
  .r.on { background: var(--accent-soft); color: var(--text); }
  .i { width: 8px; height: 8px; border-radius: 50%; background: var(--text-3); margin-left: 4px; }
  .m { font-family: var(--figures); font-size: 12px; color: var(--text-3); }
  em { font-style: normal; color: var(--accent); }
  .none { padding: 12px 16px; margin: 0; }
  .foot { display: flex; gap: 16px; padding: 8px 16px; border-top: 1px solid var(--border); font-size: 11px; color: var(--text-3); }
  .ok { margin-left: auto; color: var(--positive); }
</style>
