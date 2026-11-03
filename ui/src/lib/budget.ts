// Budget page layout: pure, so it is tested. Amounts arrive as decimal strings.
import type { BudgetView, CategoryLine, Txn, TxnStatus } from "./api";

const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/** "2026-11-01" -> "November 2026". */
export function monthLabel(iso: string): string {
  const [y, m] = iso.split("-").map(Number);
  return `${MONTHS[m - 1]} ${y}`;
}

/** "2026-11-01", -1 -> "2026-10". */
export function shiftMonth(iso: string, by: number): string {
  const [y, m] = iso.split("-").map(Number);
  const i = y * 12 + (m - 1) + by;
  return `${Math.floor(i / 12)}-${String((i % 12) + 1).padStart(2, "0")}`;
}

export const STATUS_LABEL: Record<TxnStatus, string> = {
  provisional: "Apple Pay tap, waiting for the statement",
  confirmed: "Tap matched to the statement",
  posted: "From the statement",
  review: "No statement row a week after the tap",
  tap_only: "Apple Pay tap (this card has no statements)",
  unlinked: "Tap from a card not linked yet",
};

/** One line under the hero: how the month is going against an even pace. */
export function paceLine(v: BudgetView): { left: number; perDay: number | null; vsPace: number } {
  const budget = Number(v.budget);
  const spent = Number(v.spent);
  const daysLeft = v.days - v.day;
  return {
    left: budget - spent,
    perDay: daysLeft > 0 && budget > spent ? (budget - spent) / daysLeft : null,
    vsPace: spent - Number(v.pace),
  };
}

/** Widths for a bullet bar, in percent of the track. The track is the larger of the
 *  limit and what was spent, so an overspend shows as a red tail past the limit mark. */
export function bullet(line: Pick<CategoryLine, "spent" | "provisional" | "limit">, paceFrac: number) {
  const spent = Math.max(0, Number(line.spent));
  const prov = Math.min(spent, Math.max(0, Number(line.provisional)));
  const limit = line.limit != null ? Number(line.limit) : null;
  const track = Math.max(limit ?? 0, spent) || 1;
  const within = limit != null ? Math.min(spent, limit) : spent;
  const over = limit != null ? Math.max(0, spent - limit) : 0;
  // Confirmed money fills first; taps not on a statement yet sit at the end.
  const confirmedWithin = Math.min(spent - prov, within);
  const provWithin = within - confirmedWithin;
  return {
    confirmed: (confirmedWithin / track) * 100,
    provisional: (provWithin / track) * 100,
    over: (over / track) * 100,
    limitAt: limit != null ? (limit / track) * 100 : null,
    paceAt: limit != null ? ((limit * paceFrac) / track) * 100 : null,
    isOver: over > 0,
  };
}

export interface Chart {
  confirmed: string;
  total: string | null;
  pace: string | null;
  bars: { x: number; w: number; h: number; day: number; value: number; clipped: boolean }[];
  today: { x: number; y: number } | null;
  yTicks: { y: number; value: number }[];
  max: number;
  x: (day: number) => number;
  y: (v: number) => number;
}

/** Running total (confirmed solid, with taps dashed above it), even pace to the budget,
 *  and a separate strip of daily bars on its own scale. Days are 1-based on the x axis. */
export function chart(v: BudgetView, w: number, h: number, strip = 34): Chart {
  const L = 8, R = 56, T = 12;
  const plotH = h - T - strip - 22;
  const cum = v.cumulative.map((c) => (c == null ? null : Number(c)));
  const prov = v.cumulative_provisional.map((c) => (c == null ? null : Number(c)));
  const budget = Number(v.budget);
  const top = Math.max(budget, ...cum.map((c) => c ?? 0), 1);
  const max = niceCeil(top * 1.05);
  const x = (day: number) => L + ((w - L - R) * day) / v.days;
  const y = (val: number) => T + plotH * (1 - val / max);
  const pts = (vals: (number | null)[]) => {
    const out: string[] = [`M${x(0).toFixed(1)} ${y(0).toFixed(1)}`];
    vals.forEach((c, i) => { if (c != null) out.push(`L${x(i + 1).toFixed(1)} ${y(c).toFixed(1)}`); });
    return out.join(" ");
  };
  const confirmed = pts(cum.map((c, i) => (c == null ? null : c - (prov[i] ?? 0))));
  const anyProv = prov.some((p) => (p ?? 0) > 0);
  let total: string | null = null;
  if (anyProv) {
    const first = prov.findIndex((p) => (p ?? 0) > 0);
    const seg: string[] = [];
    const startVal = first === 0 ? 0 : (cum[first - 1] ?? 0);
    seg.push(`M${x(first).toFixed(1)} ${y(startVal).toFixed(1)}`);
    for (let i = first; i < cum.length; i++) if (cum[i] != null) seg.push(`L${x(i + 1).toFixed(1)} ${y(cum[i]!).toFixed(1)}`);
    total = seg.join(" ");
  }
  const lastDay = cum.reduce<number>((a, c, i) => (c != null ? i + 1 : a), 0);
  const daily = v.daily.map((d) => (d == null ? null : Number(d)));
  // One rent day would flatten every other bar; scale to the busy-day level instead
  // and clip the outliers (they are marked, and the hover shows the real number).
  const sorted = daily.filter((d): d is number => d != null && d > 0).sort((a, b) => a - b);
  const p90 = sorted.length ? sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.9))] : 1;
  const dmax = Math.max(1, Math.min(sorted[sorted.length - 1] ?? 1, p90 * 1.5));
  const slot = (w - L - R) / v.days;
  const bars = daily.flatMap((d, i) =>
    d == null || d <= 0
      ? []
      : [{ x: x(i) + slot * 0.18, w: Math.max(1, slot * 0.64), h: (strip - 4) * Math.min(1, d / dmax), day: i + 1, value: d, clipped: d > dmax }],
  );
  const ticks = [0, max / 2, max].map((value) => ({ y: y(value), value }));
  return {
    confirmed,
    total,
    // Bills step up on their day; the rest of the budget is a straight line.
    pace: budget > 0 ? [`M${x(0).toFixed(1)} ${y(0).toFixed(1)}`, ...v.pace_by_day.map((p, i) => `L${x(i + 1).toFixed(1)} ${y(Number(p)).toFixed(1)}`)].join(" ") : null,
    bars,
    today: lastDay > 0 ? { x: x(lastDay), y: y(cum[lastDay - 1] ?? 0) } : null,
    yTicks: ticks,
    max,
    x,
    y,
  };
}

function niceCeil(n: number): number {
  if (n <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(n));
  for (const m of [1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10]) if (m * p >= n) return m * p;
  return 10 * p;
}

/** Transactions grouped by day, newest first, with each day's spending total. */
export function byDay(txns: Txn[], spending: (t: Txn) => boolean): { date: string; total: number; txns: Txn[] }[] {
  const out: { date: string; total: number; txns: Txn[] }[] = [];
  for (const t of txns) {
    let g = out[out.length - 1];
    if (!g || g.date !== t.date) { g = { date: t.date, total: 0, txns: [] }; out.push(g); }
    g.txns.push(t);
    if (spending(t)) g.total += Number(t.base_amount ?? 0);
  }
  return out;
}

export const counts = (category: string) => category !== "income" && category !== "transfer";

// ---- Draft: "days on pace" and "months kept" (gamification exploration) ----

export type PaceDay = { day: number; state: "under" | "over" | "today-under" | "today-over" | "future"; noSpend: boolean; total: number; pace: number; spent: number };

/** Each day of the month against the pace line: on pace means the running total at the
 *  end of the day (taps included) was at or under where the budget said you'd be. */
export function paceDays(v: BudgetView): { days: PaceDay[]; run: number; best: number; under: number; noSpend: number; overSince: number | null } {
  const days: PaceDay[] = [];
  for (let i = 0; i < v.days; i++) {
    const c = v.cumulative[i];
    const pace = Number(v.pace_by_day[i] ?? 0);
    if (c == null || i + 1 > v.day) { days.push({ day: i + 1, state: "future", noSpend: false, total: 0, pace, spent: 0 }); continue; }
    const total = Number(c);
    const spent = Number(v.daily[i] ?? 0);
    const ok = total <= pace;
    const today = i + 1 === v.day && v.day < v.days;
    days.push({ day: i + 1, state: today ? (ok ? "today-under" : "today-over") : ok ? "under" : "over", noSpend: spent <= 0, total, pace, spent });
  }
  const past = days.filter((d) => d.state !== "future");
  const ok = (d: PaceDay) => d.state === "under" || d.state === "today-under";
  let run = 0;
  for (let i = past.length - 1; i >= 0 && ok(past[i]); i--) run++;
  let best = 0, cur = 0;
  for (const d of past) { cur = ok(d) ? cur + 1 : 0; best = Math.max(best, cur); }
  let overSince: number | null = null;
  if (run === 0 && past.length) { let i = past.length - 1; while (i > 0 && !ok(past[i - 1])) i--; overSince = past[i].day; }
  return { days, run, best, under: past.filter(ok).length, noSpend: past.filter((d) => d.noSpend).length, overSince };
}

/** The five months before this one against today's budget (draft: uses current limits). */
export function monthsKept(v: BudgetView): { month: string; spent: number; budget: number; kept: boolean; current: boolean }[] {
  const budget = Number(v.budget);
  const out = [];
  for (let k = 5; k >= 1; k--) {
    const spent = v.categories.reduce((a, c) => a + Number(c.history[5 - k] ?? 0), 0);
    out.push({ month: shiftMonth(v.month, -k), spent, budget, kept: spent <= budget, current: false });
  }
  out.push({ month: v.month.slice(0, 7), spent: Number(v.spent), budget: Number(v.pace), kept: Number(v.spent) <= Number(v.pace), current: true });
  return out.filter((m) => m.current || m.spent > 0);
}

// ---- Draft 2: the round game. Paydays and goals are mocked here; a build would read
// them from the ledger (settings.paydays, settings.round_goal, recurring contributions).

export interface RoundDay { day: number; iso: string; allowance: number; spent: number; closed: boolean; today: boolean; net: number }
export interface Round {
  start: string; end: string; payday: string; n: number; of: number; days: number; dayIn: number;
  allowance: number; days_: RoundDay[]; banked: number; taken: number; planned: number; goal: number; pot: number; toGo: number; daysLeft: number;
  today: { allowance: number; spent: number; left: number; taps: { merchant: string; amount: number; category: string }[] };
}

const PAYDAYS = [15, 30]; // mocked "paid twice a month" setting
const dayIso = (month: string, d: number) => `${month.slice(0, 7)}-${String(d).padStart(2, "0")}`;

export function round(v: BudgetView, goal = 500, planned = 250): Round {
  const bills = v.categories.find((c) => c.category === "bills");
  const disc = Number(v.budget) - Number(bills?.limit ?? 0);
  const allowance = Math.round(disc / v.days);
  const today = v.day;
  const start = today >= PAYDAYS[0] ? PAYDAYS[0] : 1;
  const end = today >= PAYDAYS[0] ? v.days - 1 : PAYDAYS[0] - 1;
  const paidOn = new Map(v.bills.filter((b) => b.paid_on).map((b) => [b.paid_on!, Number(b.paid_amount ?? b.amount)]));
  const days_: RoundDay[] = [];
  for (let d = start; d <= end; d++) {
    const iso = dayIso(v.month, d);
    const raw = d <= today ? Number(v.daily[d - 1] ?? 0) - (paidOn.get(iso) ?? 0) : 0;
    const spent = Math.max(0, raw);
    days_.push({ day: d, iso, allowance, spent, closed: d < today, today: d === today, net: d < today ? allowance - spent : 0 });
  }
  const banked = days_.filter((d) => d.closed && d.net > 0).reduce((a, d) => a + d.net, 0);
  const taken = days_.filter((d) => d.closed && d.net < 0).reduce((a, d) => a - d.net, 0);
  const pot = Math.max(0, planned + banked - taken);
  const taps = v.txns.filter((t) => t.date === dayIso(v.month, today) && counts(t.category)).reverse()
    .map((t) => ({ merchant: t.merchant, amount: Number(t.base_amount ?? t.amount), category: t.category }));
  const spentToday = taps.reduce((a, t) => a + t.amount, 0);
  const [y, m] = v.month.split("-").map(Number);
  return {
    start: dayIso(v.month, start), end: dayIso(v.month, end), payday: end + 1 > v.days ? shiftMonth(v.month, 1) + "-01" : dayIso(v.month, end + 1),
    n: (m - 1) * 2 + (start === 1 ? 1 : 2), of: 24, days: days_.length, dayIn: today - start + 1,
    allowance, days_, banked, taken, planned, goal, pot, toGo: Math.max(0, goal - pot), daysLeft: end - today,
    today: { allowance, spent: spentToday, left: allowance - spentToday, taps },
  };
}

/** Arc segments for the day ring: one wedge per tap, in order, then what is left. */
export function ringWedges(allowance: number, taps: { amount: number }[], r = 44, cx = 50, cy = 50) {
  const total = Math.max(allowance, taps.reduce((a, t) => a + t.amount, 0)) || 1;
  const C = 2 * Math.PI * r;
  let acc = 0;
  const seg = (from: number, to: number) => {
    const a = (f: number) => -Math.PI / 2 + 2 * Math.PI * f;
    const p = (f: number) => `${(cx + r * Math.cos(a(f))).toFixed(2)} ${(cy + r * Math.sin(a(f))).toFixed(2)}`;
    const large = to - from > 0.5 ? 1 : 0;
    return `M${p(from)} A${r} ${r} 0 ${large} 1 ${p(Math.min(to, 0.9999))}`;
  };
  const wedges = taps.map((t) => { const from = acc / total; acc += t.amount; return { d: seg(from, acc / total), over: acc > allowance }; });
  return { wedges, left: seg(acc / total, 1), circumference: C, overFrac: Math.max(0, (acc - allowance) / total) };
}
