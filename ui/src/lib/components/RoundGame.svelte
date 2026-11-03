<script lang="ts">
  // DRAFT (gamification exploration). Three variants of the same loop, picked with
  // localStorage["ww-gamify"]: "rings" | "hearts" | "race". Season, XP, ghost and
  // challenge figures are mocked where the ledger has no field for them yet.
  import type { BudgetView } from "../api";
  import { monthLabel, ringWedges, round } from "../budget";
  import { formatMoney } from "../money";
  import { prefs } from "../stores/prefs.svelte";

  let { v }: { v: BudgetView } = $props();
  const variant = (typeof localStorage !== "undefined" && localStorage.getItem("ww-gamify")) || "rings";
  const r = $derived(round(v));
  const money = (n: number, d = 0) => (prefs.blur ? "*****" : formatMoney(n, v.base_currency, d));
  const mon = $derived(monthLabel(v.month).split(" ")[0].slice(0, 3));
  const md = (iso: string) => `${monthLabel(iso.slice(0, 7) + "-01").slice(0, 3)} ${Number(iso.slice(8))}`;
  const ring = $derived(ringWedges(r.today.allowance, r.today.taps));

  // ---- mocked season (rounds 1..21 of 2026): saved vs a $500 goal ----
  const SEASON = [520, 500, 410, 560, 500, 640, 500, 380, 500, 700, 520, 500, 500, 455, 610, 500, 580, 500, 540, 500, 620];
  const hit = SEASON.filter((s) => s >= 500).length;
  const vault = SEASON.reduce((a, s) => a + Math.max(0, s - 500), 0);
  const potPct = $derived(Math.min(100, (r.pot / r.goal) * 100));
  const plannedPct = $derived(Math.min(potPct, (r.planned / r.goal) * 100));
  const dmax = $derived(Math.max(r.allowance, ...r.days_.map((d) => Math.abs(d.net))) || 1);

  // ---- hearts variant: 3 lives per round, one lost per overspent day; XP = dollars banked ----
  const lost = $derived(r.days_.filter((d) => d.closed && d.net < 0).length);
  const XP = 1240 + Math.round(r.banked); // mocked season XP carried in
  const LEVELS = [0, 200, 500, 900, 1500, 2300, 3300, 4600];
  const level = LEVELS.filter((l) => XP >= l).length;
  const next = LEVELS[level] ?? LEVELS[LEVELS.length - 1] * 1.4;
  const prev = LEVELS[level - 1] ?? 0;
  const week = $derived(r.days_.filter((d) => d.closed).slice(-7).reduce((a, d) => a + Math.max(0, d.net), 0));

  // ---- race variant: this round's running discretionary spend vs last round (ghost) ----
  const GHOST = [61, 118, 203, 247, 310, 402, 433, 520, 588, 640, 701, 790, 851, 902, 990];
  const cum = $derived(r.days_.reduce<number[]>((a, d, i) => (d.closed || d.today ? [...a, (a[i - 1] ?? 0) + d.spent] : a), []));
  const W = 320, H = 110, L = 6, B = 18;
  const top = $derived(Math.max(GHOST[GHOST.length - 1], r.allowance * r.days) * 1.05);
  const x = (i: number) => L + ((W - L - 6) * i) / r.days;
  const y = (val: number) => 6 + (H - B - 6) * (1 - val / top);
  const path = (vals: number[]) => vals.map((c, i) => `${i ? "L" : "M"}${x(i + 1).toFixed(1)} ${y(c).toFixed(1)}`).join(" ");
  const ahead = $derived(GHOST[cum.length - 1] - cum[cum.length - 1]);
  const CHALLENGES = [
    { name: "No dining on weekdays", sub: "this week", done: 3, of: 5, reward: 40, unit: "days" },
    { name: "Coffee under $25", sub: "this week", done: 18.75, of: 25, reward: 10, unit: "$" },
    { name: "Round-ups to TFSA", sub: "this round", done: 23.4, of: 40, reward: 0, unit: "$" },
  ];
  const GOAL = { name: "Japan, April", saved: 2650, target: 4000, next: 3000, by: "Dec 15" };
</script>

<section class="game" data-variant={variant}>
  <div class="hrow">
    <h2 class="label">Round {r.n} of {r.of} <span class="faint">· {md(r.start)} → {md(r.end)} · payday {md(r.payday)} · day {r.dayIn} of {r.days}</span></h2>
    <span class="faint small">
      {#if variant === "rings"}each tap takes a wedge off today · leftover banks at midnight · overspend comes out of the pot
      {:else if variant === "hearts"}an overspent day costs a life · banked dollars are XP
      {:else}race your last round · challenges pay into the pot{/if}
    </span>
  </div>

  {#if variant === "rings"}
    <div class="grid rings">
      <div class="today">
        <svg viewBox="0 0 100 100" class="ring" aria-label="Today's allowance">
          <circle cx="50" cy="50" r="44" class="track" />
          <path d={ring.left} class="left" />
          {#each ring.wedges as w}<path d={w.d} class="wedge" class:over={w.over} />{/each}
          <text x="50" y="47" class="big money">{money(Math.max(0, r.today.left))}</text>
          <text x="50" y="61" class="sm">{r.today.left >= 0 ? "left today" : `${money(-r.today.left)} over`}</text>
        </svg>
        <div class="taps">
          <div class="faint small">of {money(r.today.allowance)} · {r.today.taps.length} tap{r.today.taps.length === 1 ? "" : "s"}</div>
          {#each r.today.taps as t}<div class="tap"><span>{t.merchant}</span><span class="money">{money(t.amount, 2)}</span></div>{/each}
          <div class="faint small">banks at midnight →</div>
        </div>
      </div>
      <div class="pot">
        <div class="potline">
          <span><b class="money">{money(r.pot)}</b> in the pot</span>
          {#if r.toGo > 0}<span><b class="money">{money(r.toGo)}</b> to go · {r.daysLeft} days</span>{:else}<span class="pos">goal hit · overflow counts</span>{/if}
        </div>
        <div class="bar" title="goal {money(r.goal)}: {money(r.planned)} planned + {money(r.banked)} banked − {money(r.taken)} taken">
          <i class="planned" style="width:{plannedPct}%"></i><i class="banked" style="width:{potPct - plannedPct}%"></i>
          <span class="goal" style="left:100%"></span>
        </div>
        <div class="ml"><span><i class="sw planned"></i>planned {money(r.planned)}</span><span><i class="sw banked"></i>banked {money(r.banked)}{#if r.taken > 0} · <span class="neg">{money(r.taken)} taken</span>{/if}</span><span>goal {money(r.goal)}</span></div>
        <div class="days" style="grid-template-columns: repeat({r.days}, 1fr)">
          {#each r.days_ as d}
            <div class="dcol" class:now={d.today} class:closed={d.closed} title="{md(d.iso)}: {d.closed ? `${money(d.spent)} of ${money(d.allowance)} · ${d.net >= 0 ? 'banked' : 'took'} ${money(Math.abs(d.net))}` : d.today ? 'today' : ''}">
              <div class="up">{#if d.closed && d.net > 0}<i style="height:{(d.net / dmax) * 100}%"></i>{/if}</div>
              <div class="dn">{#if d.closed && d.net < 0}<i style="height:{(-d.net / dmax) * 100}%"></i>{/if}</div>
              <span class="dl">{d.day}</span>
            </div>
          {/each}
        </div>
        <div class="reward">Hit it: <b>target date 2 days closer</b>. Every <span class="money">$100</span> over: one more.</div>
      </div>
      <div class="season">
        <div class="label">Season 2026</div>
        <div class="tiles">
          {#each SEASON as s}<i class:miss={s < 500} style="height:{Math.min(100, 40 + ((s - 500) / 200) * 60)}%" title="{money(s)} of $500"></i>{/each}
          <i class="cur" style="height:{40 + Math.min(60, (r.pot / r.goal) * 40)}%" title="this round"></i>
          {#each { length: r.of - SEASON.length - 1 } as _}<i class="todo"></i>{/each}
        </div>
        <div class="sub"><b>{hit} of {SEASON.length}</b> rounds hit · <span class="money pos">{money(vault)}</span> in the vault<br /><span class="faint">target date</span> <s class="faint">Mar 2041</s> <b>Feb 2041</b> <a href="#/plan" class="small">Plan ›</a></div>
      </div>
    </div>

  {:else if variant === "hearts"}
    <div class="grid hearts">
      <div class="lives">
        <div class="hearts-row" aria-label="{3 - lost} of 3 lives">
          {#each [0, 1, 2] as i}<span class="heart" class:lost={i >= 3 - lost}>♥</span>{/each}
          <span class="faint small">lives this round</span>
        </div>
        <div class="allow">
          <div class="potline"><span>Today <b class="money">{money(Math.max(0, r.today.left))}</b> left</span><span class="faint">of {money(r.today.allowance)}</span></div>
          <div class="bar" class:danger={r.today.left < r.today.allowance * 0.2}>
            {#each r.today.taps as t}<i class="tapseg" style="width:{(t.amount / r.today.allowance) * 100}%" title="{t.merchant} {money(t.amount, 2)}"></i>{/each}
            <span class="zone" style="left:80%"></span>
          </div>
          <div class="ml"><span class="faint">go over and a life is gone at midnight</span><span class="faint">danger zone</span></div>
        </div>
        <div class="freeze"><span class="tok">❄</span> 1 freeze left <span class="faint">· forgives one bad day</span></div>
      </div>
      <div class="xp">
        <div class="lvl"><span class="lv">Lv {level}</span><div><b>Steady Saver</b><br /><span class="faint small">1 XP for every dollar banked or saved over goal</span></div></div>
        <div class="bar"><i class="banked" style="width:{((XP - prev) / (next - prev)) * 100}%"></i></div>
        <div class="ml"><span class="money">{XP.toLocaleString()} XP</span><span class="faint">Lv {level + 1} at {next.toLocaleString()} · {(next - XP).toLocaleString()} to go</span></div>
        <div class="quest">
          <div class="potline"><span>Weekly quest · bank <b class="money">$150</b></span><span class="money">{money(week)} / $150</span></div>
          <div class="bar"><i class="planned" style="width:{Math.min(100, (week / 150) * 100)}%"></i></div>
          <div class="ml"><span class="faint">3 days left</span><span class="faint">reward: +50 XP</span></div>
        </div>
      </div>
      <div class="streak">
        <div class="flame">🔥</div>
        <div class="n">23</div>
        <div class="sub">day streak<br /><span class="faint">longest 41 · a day counts when it closes under allowance</span></div>
        <div class="week">{#each ["M", "T", "W", "T", "F", "S", "S"] as d, i}<span class:on={i < 3} class:cur={i === 3}>{d}</span>{/each}</div>
      </div>
    </div>

  {:else}
    <div class="grid race">
      <div class="ghost">
        <div class="potline"><span>This round vs last</span><span class:pos={ahead >= 0} class:neg={ahead < 0}><b class="money">{money(Math.abs(ahead))}</b> {ahead >= 0 ? "ahead" : "behind"}</span></div>
        <svg viewBox="0 0 {W} {H}" class="chart" aria-label="Running spend vs last round">
          <line x1={x(0)} y1={y(0)} x2={x(r.days)} y2={y(r.allowance * r.days)} class="allow" />
          <path d={path(GHOST)} class="gline" />
          <path d={path(cum)} class="me" />
          <circle cx={x(cum.length)} cy={y(cum[cum.length - 1])} r="3" class="dot" />
          <circle cx={x(cum.length)} cy={y(GHOST[cum.length - 1])} r="2.5" class="gdot" />
          <text x={x(1)} y={H - 4} class="ax">{md(r.start)}</text><text x={x(r.days)} y={H - 4} class="ax end">{md(r.end)}</text>
        </svg>
        <div class="legend"><span><i class="ln me"></i>you</span><span><i class="ln ghost"></i>last round (ghost)</span><span><i class="ln allow"></i>allowance</span></div>
      </div>
      <div class="challenges">
        {#each CHALLENGES as c}
          <div class="ch">
            <div class="potline"><span><b>{c.name}</b> <span class="faint small">{c.sub}</span></span><span class="faint small">{c.reward ? `+${money(c.reward)} to pot` : "moves real money"}</span></div>
            <div class="bar"><i class="planned" style="width:{Math.min(100, (c.done / c.of) * 100)}%"></i></div>
            <div class="ml"><span class="money">{c.unit === "$" ? money(c.done, 2) : c.done} / {c.unit === "$" ? money(c.of) : `${c.of} ${c.unit}`}</span></div>
          </div>
        {/each}
        <button type="button" class="link small">+ pick a challenge</button>
      </div>
      <div class="goal">
        <div class="therm"><i style="height:{(GOAL.saved / GOAL.target) * 100}%"></i><span class="mark" style="bottom:{(GOAL.next / GOAL.target) * 100}%"></span></div>
        <div>
          <div class="label">{GOAL.name}</div>
          <div class="big2 money">{money(GOAL.saved)}</div>
          <div class="sub">of {money(GOAL.target)}<br /><span class="faint">next milestone {money(GOAL.next)} by {GOAL.by}</span><br /><span class="faint">this round adds</span> <b class="money">{money(r.pot)}</b></div>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .game { border: 1px solid var(--border); border-radius: var(--radius); padding: 14px 18px 16px; background: var(--surface); }
  .hrow { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; flex-wrap: wrap; margin-bottom: 14px; }
  .hrow h2 { margin: 0; font-weight: 500; }
  .small { font-size: 12px; }
  .pos { color: var(--positive); }
  .neg { color: var(--negative); }
  .sub { font-size: 12px; color: var(--text-2); line-height: 1.5; }
  .sub b, .potline b { font-weight: 500; color: var(--text); }
  .grid { display: grid; gap: 28px; align-items: start; }
  .rings { grid-template-columns: 300px 1fr 230px; }
  .hearts { grid-template-columns: 1fr 1fr 170px; }
  .race { grid-template-columns: 340px 1fr 220px; }
  /* shared bars */
  .potline { display: flex; justify-content: space-between; font-size: 13px; color: var(--text-2); margin-bottom: 6px; }
  .bar { height: 10px; background: var(--surface-2); border-radius: 5px; position: relative; display: flex; overflow: visible; }
  .bar i { display: block; height: 100%; }
  .bar i:first-child { border-radius: 5px 0 0 5px; }
  .bar i:last-of-type { border-radius: 0 5px 5px 0; }
  .planned { background: var(--fill); }
  .banked { background: var(--accent); }
  .bar .goal { position: absolute; top: -4px; bottom: -4px; width: 2px; background: var(--text); margin-left: -2px; }
  .ml { display: flex; justify-content: space-between; gap: 10px; font-size: 11px; color: var(--text-3); margin-top: 6px; flex-wrap: wrap; }
  .ml span { display: inline-flex; align-items: center; gap: 5px; }
  .sw { width: 10px; height: 8px; border-radius: 2px; display: inline-block; }
  /* rings */
  .today { display: flex; gap: 14px; align-items: center; }
  .ring { width: 130px; height: 130px; flex: none; }
  .ring .track { fill: none; stroke: var(--surface-2); stroke-width: 9; }
  .ring .left { fill: none; stroke: var(--positive); stroke-width: 9; stroke-linecap: butt; }
  .ring .wedge { fill: none; stroke: var(--fill); stroke-width: 9; }
  .ring .wedge.over { stroke: var(--negative); }
  .ring .big { font-family: var(--display); font-size: 17px; fill: var(--text); text-anchor: middle; }
  .ring .sm { font-size: 7px; fill: var(--text-3); text-anchor: middle; font-family: var(--font); }
  .taps { font-size: 12px; color: var(--text-2); display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .tap { display: flex; justify-content: space-between; gap: 10px; border-bottom: 1px solid var(--border); padding: 2px 0; }
  .days { display: grid; gap: 3px; margin-top: 14px; }
  .dcol { display: flex; flex-direction: column; align-items: center; }
  .dcol .up, .dcol .dn { width: 100%; height: 22px; display: flex; align-items: flex-end; }
  .dcol .dn { align-items: flex-start; }
  .dcol i { display: block; width: 100%; border-radius: 2px; background: var(--positive); }
  .dcol .dn i { background: var(--negative); }
  .dcol .dl { font-size: 9px; color: var(--text-3); font-family: var(--mono); margin-top: 2px; }
  .dcol.now .dl { color: var(--accent); font-weight: 600; }
  .dcol.now .up { box-shadow: inset 0 -1px 0 var(--accent); }
  .reward { font-size: 12px; color: var(--text-2); margin-top: 10px; }
  .reward b { color: var(--accent); font-weight: 500; }
  .season .tiles { display: flex; gap: 3px; align-items: flex-end; height: 40px; margin: 8px 0 8px; }
  .season .tiles i { flex: 1; background: var(--accent); border-radius: 2px 2px 0 0; display: block; }
  .season .tiles i.miss { background: transparent; box-shadow: inset 0 0 0 1px var(--negative); }
  .season .tiles i.cur { background: color-mix(in oklab, var(--accent) 45%, var(--surface-2)); box-shadow: inset 0 0 0 1px var(--accent); }
  .season .tiles i.todo { background: transparent; box-shadow: inset 0 0 0 1px var(--border); height: 40% !important; }
  /* hearts */
  .hearts-row { display: flex; gap: 4px; align-items: center; font-size: 22px; margin-bottom: 12px; }
  .heart { color: var(--negative); }
  .heart.lost { color: var(--border); }
  .hearts-row .faint { margin-left: 8px; }
  .tapseg { background: var(--fill); border-right: 2px solid var(--surface); }
  .bar.danger .tapseg { background: var(--negative); }
  .bar .zone { position: absolute; top: 0; bottom: 0; right: 0; left: 80%; background: repeating-linear-gradient(135deg, color-mix(in oklab, var(--negative) 35%, transparent) 0 3px, transparent 3px 6px); border-radius: 0 5px 5px 0; }
  .freeze { font-size: 12px; color: var(--text-2); margin-top: 12px; }
  .tok { display: inline-block; border: 1px solid var(--border); border-radius: 999px; padding: 0 7px; margin-right: 4px; }
  .lvl { display: flex; gap: 12px; align-items: center; margin-bottom: 8px; font-size: 13px; }
  .lv { font-family: var(--display); font-size: 26px; color: var(--accent); }
  .quest { margin-top: 16px; }
  .streak { text-align: center; }
  .flame { font-size: 22px; }
  .streak .n { font-family: var(--display); font-size: 3rem; line-height: 1; color: var(--accent); }
  .week { display: flex; gap: 4px; justify-content: center; margin-top: 8px; }
  .week span { width: 18px; height: 18px; border-radius: 50%; font-size: 10px; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--border); color: var(--text-3); }
  .week span.on { background: var(--accent); color: var(--bg); border-color: var(--accent); }
  .week span.cur { border-color: var(--accent); color: var(--accent); }
  /* race */
  .chart { width: 100%; height: auto; display: block; }
  .chart .allow { stroke: var(--text-3); stroke-dasharray: 2 3; stroke-width: 1; }
  .chart .gline { fill: none; stroke: var(--text-3); stroke-width: 1.5; stroke-dasharray: 4 3; opacity: 0.8; }
  .chart .me { fill: none; stroke: var(--accent); stroke-width: 2; }
  .chart .dot { fill: var(--accent); }
  .chart .gdot { fill: var(--text-3); }
  .chart .ax { font-size: 9px; fill: var(--text-3); font-family: var(--mono); }
  .chart .ax.end { text-anchor: end; }
  .legend { display: flex; gap: 14px; font-size: 11px; color: var(--text-2); margin-top: 4px; }
  .legend span { display: inline-flex; align-items: center; gap: 5px; }
  .ln { width: 14px; border-top: 2px solid var(--accent); display: inline-block; }
  .ln.ghost { border-top: 2px dashed var(--text-3); }
  .ln.allow { border-top: 1px dotted var(--text-3); }
  .ch { margin-bottom: 12px; }
  .link { background: none; border: 0; color: var(--accent); font: inherit; cursor: pointer; padding: 0; }
  .goal { display: flex; gap: 14px; align-items: stretch; }
  .therm { width: 14px; min-height: 110px; background: var(--surface-2); border-radius: 7px; position: relative; display: flex; align-items: flex-end; overflow: hidden; flex: none; }
  .therm i { display: block; width: 100%; background: var(--accent); }
  .therm .mark { position: absolute; left: 0; right: 0; height: 2px; background: var(--text); }
  .big2 { font-family: var(--display); font-size: 1.9rem; line-height: 1.1; margin: 2px 0; }
  @media (max-width: 760px) { .grid { grid-template-columns: 1fr !important; } }
</style>
