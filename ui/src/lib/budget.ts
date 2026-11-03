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
  manual: "Added by hand",
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
