<script lang="ts">
  // The budget game: the month as a track, the jar it fills, and this week's cards.
  // Setup (limits, bills, statements) stays on Budget; this screen only plays.
  import { apply, ApiError, type GameCard, type LedgerView, type Mutation } from "../lib/api";
  import { monthLabel, shiftMonth } from "../lib/budget";
  import { daysLeft, jarFraction, paydayIn } from "../lib/game";
  import { formatMoney, parseAmount, todayIso } from "../lib/money";
  import { game } from "../lib/stores/game.svelte";
  import { view as viewStore } from "../lib/stores/view.svelte";
  import Track from "../lib/components/game/Track.svelte";
  import Jar from "../lib/components/game/Jar.svelte";
  import PlayingCard from "../lib/components/game/PlayingCard.svelte";
  import Glyph from "../lib/components/game/Glyph.svelte";
  import { glyph } from "../lib/game";

  let { view, month }: { view: LedgerView; month: string | null } = $props();
  $effect(() => { void game.ensure(view.version, month); });
  const g = $derived(game.current);
  const cur = $derived(g?.base_currency ?? view.base_currency);
  const money = (n: number, digits = 0) => formatMoney(n, cur, digits);
  const mon = $derived(g ? monthLabel(g.month).split(" ")[0].slice(0, 3) : "");
  const monthName = $derived(g ? monthLabel(g.month).split(" ")[0] : "");
  const thisMonth = todayIso().slice(0, 7);
  const shown = $derived(g?.month.slice(0, 7) ?? thisMonth);
  const canNext = $derived(shown < thisMonth);
  const canPrev = $derived(g?.first_month != null && g.first_month.slice(0, 7) < shown);
  const jar = $derived(g ? Number(g.jar) : 0);
  const goal = $derived(g?.goal != null ? Number(g.goal) : null);
  const pay = $derived(g ? paydayIn(g) : null);
  const savedOnCards = $derived(g ? g.shelf.reduce((a, c) => a + Number(c.saved), 0) : 0);

  let err = $state<string | null>(null);
  let custom = $state("");
  async function save(ms: Mutation[]) {
    err = null;
    try {
      viewStore.set(await apply(ms));
      void game.ensure(view.version + 1, month, true);
    } catch (e) {
      err = e instanceof ApiError ? e.message : String(e);
    }
  }
  const setGoal = (v: string | null) => save([{ op: "set_game_goal", goal: v }]);
  function setCustom(e: Event) {
    e.preventDefault();
    const v = parseAmount(custom);
    if (v == null || Number(v) <= 0) { err = "A goal is a dollar amount above 0."; return; }
    void setGoal(v);
  }
  const play = (c: GameCard) => save([{ op: "play_card", card: { id: c.id, week: c.week, from: c.from, rule: c.rule } }]);
  const pass = (c: GameCard) => save([{ op: "pass_card", id: c.id }]);
  const nextMonday = $derived.by(() => {
    const d = new Date(todayIso() + "T00:00:00Z");
    d.setUTCDate(d.getUTCDate() + ((8 - d.getUTCDay()) % 7 || 7));
    return d.toISOString().slice(0, 10);
  });
</script>

<div class="play">
  <header class="top">
    <div class="month">
      <a class="arrow" class:off={!canPrev} href={canPrev && g ? `#/budget/play/${shiftMonth(g.month, -1)}` : undefined} aria-label="Previous month">‹</a>
      <div>
        <h1>{g ? monthName : "Budget game"}</h1>
        {#if g && g.day >= 1 && g.day <= g.days}
          <p class="sub">day {g.day} of {g.days}{#if pay != null}{" · "}payday in {pay} day{pay === 1 ? "" : "s"}{/if}</p>
        {:else if g && g.closed}<p class="sub">month closed</p>{/if}
      </div>
      <a class="arrow" class:off={!canNext} href={canNext && g ? `#/budget/play/${shiftMonth(g.month, 1)}` : undefined} aria-label="Next month">›</a>
    </div>
    <a class="setup" href={g ? `#/budget/${g.month.slice(0, 7)}` : "#/budget"}><span class="long">Budget setup</span><span class="short">Setup</span> ›</a>
  </header>

  {#if err}<p class="err" role="alert">{err}</p>{/if}

  {#if !g}
    <p class="faint">{game.error ?? "Loading…"}</p>
  {:else if Number(g.allowance) <= 0}
    <section class="empty">
      <Jar fraction={0} size={0.8} id="coin-empty" />
      <div>
        <h2>The game needs a budget</h2>
        <p class="muted">Each day gets a share of your monthly budget, bills left out. Set a budget for a few categories first.</p>
        <a class="btn primary" href="#/budget">Set budgets</a>
      </div>
    </section>
  {:else if g.closed}
    {@const c = g.closed}
    <section class="closed">
      <p class="kicker">{monthName} closed</p>
      <Jar fraction={jarFraction(g)} goalLabel={c.goal ? money(Number(c.goal)) : null} size={1.25} id="coin-closed" glow={c.hit} />
      <div class="huge money" class:neg={jar < 0}>{money(jar)}</div>
      <p class="muted">
        {#if c.goal == null}into the jar
        {:else if c.hit}goal {money(Number(c.goal))} hit{#if jar > Number(c.goal)}, {money(jar - Number(c.goal))} over the line{/if}{#if c.streak > 1}{" · "}{c.streak} months in a row{/if}
        {:else}{money(Number(c.goal) - jar)} short of {money(Number(c.goal))}{/if}
      </p>
      <div class="stats">
        <div><b>{c.green}</b><span>green days of {c.days}</span></div>
        <div><b>{c.cards_won} / {c.cards_played}</b><span>cards won</span></div>
        <div><b class="money">{money(Math.max(0, jar - (Number(c.goal) || 0)))}</b><span>over the line</span></div>
      </div>
      <div class="mini-track"><Track {g} {money} {mon} /></div>
      {#if c.next_goal && shown === shiftMonth(thisMonth + "-01", -1)}
        <div class="next">
          <button type="button" class="btn primary" onclick={() => setGoal(c.next_goal)}>
            {Number(c.next_goal) > Number(c.goal) ? "Raise" : "Lower"} the goal to {money(Number(c.next_goal))}
          </button>
          <a class="link" href="#/budget/play">Keep {money(Number(c.goal))}</a>
        </div>
      {/if}
    </section>
  {:else}
    <section class="board">
      <Track {g} {money} {mon} />
      <aside class="jarbox">
        <Jar fraction={jarFraction(g)} goalLabel={goal != null ? money(goal) : null} id="coin-main" />
        <div class="jt">
          <div class="amt money" class:neg={jar < 0}>{money(jar)}</div>
          {#if goal != null}
            <p class="faint small">{jar >= goal ? `past the ${money(goal)} goal` : `of ${money(goal)}`} · {daysLeft(g)} days left</p>
          {:else}
            <p class="faint small">in the jar</p>
          {/if}
          {#if jar < 0}<p class="neg small">the jar is in the hole; green days fill it back</p>{/if}
        </div>
      </aside>
    </section>

    {#if goal == null}
      <section class="pick">
        <h2>Pick a goal for {monthName}</h2>
        <p class="muted small">Each day gets an allowance. What you don't spend drops into the jar at midnight. How full do you want it by the {g.days}{g.days === 31 ? "st" : "th"}?</p>
        <div class="choices">
          {#each g.suggestions as s, i}
            <button type="button" class="choice" onclick={() => setGoal(s)}>
              <Jar fraction={0.35 + i * 0.3} size={0.42} id="coin-pick-{i}" />
              <b class="money">{money(Number(s))}</b>
              <span class="faint small">{["easy", "steady", "stretch"][i] ?? ""}</span>
            </button>
          {/each}
          <form class="choice own" onsubmit={setCustom}>
            <label for="goal-own" class="small muted">Your own</label>
            <input id="goal-own" bind:value={custom} placeholder="$500" inputmode="decimal" />
            <button type="submit" class="btn">Set</button>
          </form>
        </div>
      </section>
    {/if}

    <section class="hand">
      <div class="hrow"><h2 class="label">This week's cards</h2><span class="faint small">play up to two · new deal Monday</span></div>
      <div class="cards">
        {#each g.hand as c, i (c.id)}<PlayingCard {c} {i} {money} />{/each}
        {#each g.offers as c, i (c.id)}<PlayingCard {c} i={i + g.hand.length} {money} onPlay={() => play(c)} onPass={() => pass(c)} />{/each}
        <div class="deck" aria-label="Next deal">
          <span class="deck-n">↻</span>
          <span>next deal<br />{monthLabel(nextMonday).split(" ")[0].slice(0, 3)} {Number(nextMonday.slice(8))}</span>
        </div>
      </div>
      {#if g.shelf.length}
        <div class="shelf">
          <span class="faint small">Won this year</span>
          {#each g.shelf.slice(0, 12) as c (c.id)}<span class="mini" title={c.title}><Glyph name={glyph(c.category)} size={15} /></span>{/each}
          <span class="faint small">{g.shelf.length} card{g.shelf.length === 1 ? "" : "s"}{#if savedOnCards > 0}{" · "}{money(savedOnCards)} under your usual{/if}</span>
        </div>
      {/if}
    </section>

    <p class="legend">
      <span><i class="dot green"></i>under that day's allowance</span>
      <span><i class="dot red"></i>over</span>
      <span><i class="dot coin"></i>coins: what went in the jar</span>
      <span><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5 l5 -5 5 5 v5 h-10z" fill="var(--text-3)" /></svg>bill day, never counts</span>
    </p>
  {/if}
</div>

<style>
  .play { display: flex; flex-direction: column; gap: 22px; }
  .top { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .month { display: flex; gap: 14px; align-items: center; }
  h1 { font-family: var(--display); font-weight: 500; font-size: 28px; margin: 0; line-height: 1.1; }
  .sub { margin: 2px 0 0; font-size: 12px; color: var(--text-3); }
  .arrow { color: var(--text-2); border: 1px solid var(--border); border-radius: 6px; padding: 0 8px; text-decoration: none; line-height: 22px; }
  .arrow.off { opacity: 0.3; pointer-events: none; }
  .setup { font-size: 13px; color: var(--text-2); text-decoration: none; border: 1px solid var(--border); border-radius: 999px; padding: 5px 12px; }
  .setup:hover { color: var(--text); }
  .setup { white-space: nowrap; }
  .short { display: none; }
  .err { color: var(--negative); margin: 0; }
  .small { font-size: 12px; }
  .neg { color: var(--negative) !important; }
  .board { display: grid; grid-template-columns: 1fr 190px; gap: 28px; align-items: center; }
  .jarbox { display: flex; flex-direction: column; align-items: center; text-align: center; }
  .jarbox p { margin: 2px 0 0; }
  .amt { font-family: var(--display); font-size: 2.4rem; line-height: 1; margin-top: 6px; }
  .pick { border: 1px dashed var(--accent); border-radius: 14px; padding: 16px 18px; }
  .pick h2 { font-family: var(--display); font-weight: 500; font-size: 20px; margin: 0; }
  .pick p { margin: 4px 0 12px; }
  .choices { display: flex; gap: 14px; flex-wrap: wrap; align-items: stretch; }
  .choice { display: flex; flex-direction: column; align-items: center; gap: 2px; background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 10px 16px; cursor: pointer; font: inherit; color: var(--text); }
  .choice:hover { border-color: var(--accent); }
  .choice b { font-family: var(--display); font-size: 20px; font-weight: 500; }
  .choice.own { justify-content: center; gap: 6px; cursor: default; }
  .choice.own input { width: 90px; background: var(--surface-2); border: 1px solid var(--border); color: var(--text); border-radius: 6px; padding: 4px 6px; font: inherit; text-align: center; }
  .hrow { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; margin-bottom: 12px; }
  .hand h2 { margin: 0; }
  .cards { display: flex; gap: 16px; overflow-x: auto; padding: 6px 12px 40px; margin: 0 -12px -26px; scroll-snap-type: x mandatory; }
  .cards > :global(*) { scroll-snap-align: start; }
  .deck { width: 120px; height: 226px; flex: none; border-radius: 14px; border: 1px dashed var(--border); display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; text-align: center; color: var(--text-3); font-size: 12px;
    background: repeating-linear-gradient(45deg, var(--surface-2) 0 6px, transparent 6px 12px); }
  .deck-n { font-size: 26px; color: var(--text-2); }
  .shelf { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-top: 4px; }
  .mini { width: 26px; height: 34px; border-radius: 5px; background: var(--gold-card); border: 1px solid var(--accent); color: var(--on-gold); display: inline-flex; align-items: center; justify-content: center; }
  .legend { display: flex; gap: 16px; flex-wrap: wrap; font-size: 11px; color: var(--text-3); margin: 0; }
  .legend span { display: inline-flex; gap: 6px; align-items: center; }
  .dot { width: 9px; height: 9px; border-radius: 50%; display: inline-block; }
  .dot.green { background: var(--positive); }
  .dot.red { background: var(--negative); }
  .dot.coin { background: radial-gradient(circle at 35% 35%, var(--coin-hi), var(--accent) 60%, var(--coin-lo)); }
  .empty { display: flex; gap: 24px; align-items: center; border-top: 1px solid var(--border); padding-top: 20px; }
  .empty h2 { font-family: var(--display); font-weight: 500; margin: 0 0 6px; }
  .btn { border: 1px solid var(--border); background: var(--surface); color: var(--text); border-radius: 8px; padding: 7px 14px; font: inherit; font-size: 13px; cursor: pointer; text-decoration: none; display: inline-block; }
  .btn.primary { background: var(--accent); border-color: var(--accent); color: var(--bg); font-weight: 600; }
  .link { color: var(--text-3); font-size: 13px; }
  .closed { display: flex; flex-direction: column; align-items: center; text-align: center; gap: 6px; }
  .kicker { font-size: 12px; letter-spacing: 0.1em; text-transform: uppercase; color: var(--text-3); margin: 0; }
  .huge { font-family: var(--display); font-size: 4rem; line-height: 1; color: var(--accent); }
  .closed .muted { margin: 0; }
  .stats { display: grid; grid-template-columns: repeat(3, minmax(0, 150px)); gap: 12px; margin-top: 14px; }
  .stats div { border: 1px solid var(--border); border-radius: 12px; padding: 10px 6px; background: var(--surface); display: flex; flex-direction: column; }
  .stats b { font-family: var(--display); font-size: 24px; font-weight: 500; }
  .stats span { font-size: 11px; color: var(--text-3); }
  .mini-track { width: 100%; max-width: 760px; margin-top: 14px; opacity: 0.9; }
  .next { display: flex; gap: 16px; align-items: center; margin-top: 10px; }
  @media (max-width: 760px) {
    .long { display: none; }
    .short { display: inline; }
    h1 { font-size: 24px; }
    .board { grid-template-columns: 1fr; }
    .jarbox { flex-direction: row; gap: 14px; text-align: left; order: -1; }
    .jarbox :global(svg) { width: 80px; height: auto; }
    .huge { font-size: 3rem; }
    .stats { grid-template-columns: repeat(3, 1fr); }
  }
</style>
