// Browser-side layout for the Plan screen and the wall view. Every dollar figure
// comes from the server; this only turns figures into proportions and words.
import type { DebtPayoff, Milestone, Point, ProjPoint, RoomView } from "./api";
import { formatCompact } from "./money";

/** A milestone split into what you have today, what you put in, and what grew. */
export interface Split { today: number; putIn: number; growth: number; total: number }
export function milestoneSplit(startNetWorth: string, m: Milestone): Split {
  const today = Math.max(0, Number(startNetWorth));
  const putIn = Math.max(0, Number(m.contributed));
  const growth = Math.max(0, Number(m.net_worth) - Number(startNetWorth) - Number(m.contributed));
  return { today, putIn, growth, total: today + putIn + growth || 1 };
}

/** Room bar segments as percentages of the year's room: used, planned (capped at what is left), left. */
export interface RoomBar { used: number; planned: number; left: number; over: number; room: number }
export function roomBar(r: RoomView): RoomBar {
  const room = Number(r.reported) + Number(r.added);
  const used = Number(r.used);
  const left = Number(r.left);
  const planned = Math.min(Number(r.planned), Math.max(0, left));
  const scale = Math.max(room, used, 1);
  const pct = (v: number) => (v / scale) * 100;
  return {
    used: pct(Math.min(used, room)),
    planned: pct(planned),
    left: pct(Math.max(0, left - planned)),
    over: left < 0 ? pct(-left) : 0,
    room,
  };
}

/** "full by Jul 2026", "$500/mo planned", "no rule" — the line under a room bar. */
export function roomLine(r: RoomView): string {
  const plan = Number(r.monthly_plan);
  if (r.over) return `over by ${formatCompact(-Number(r.left))}: CRA charges 1% a month on the excess`;
  if (plan <= 0) return "no recurring rule";
  const base = `${formatCompact(plan)}/mo planned`;
  return r.runs_out ? `${base} · full by ${monthYear(r.runs_out)}` : base;
}

export function monthYear(iso: string): string {
  const [y, m] = iso.split("-").map(Number);
  return new Intl.DateTimeFormat("en-CA", { month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(Date.UTC(y, m - 1, 1)));
}

/** Whole and fractional years between two ISO dates, one decimal. */
export function yearsBetween(from: string, to: string): number {
  const ms = Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`);
  return Math.round((ms / (365.25 * 86400000)) * 10) / 10;
}

/** The end-of-debt words under a payoff curve. */
export function payoffLine(d: DebtPayoff, years: number): string {
  if (d.payoff) return `paid off ${monthYear(d.payoff)}`;
  if (Number(d.monthly_payment) <= 0) return Number(d.rate_pct) > 0 ? "no payment rule · grows" : "no payment rule · stays flat";
  return `not paid off within ${years} years`;
}

/** Points for the fan chart: history (net worth so far) then the projection, as numbers. */
export interface Fan {
  hist: { t: number; v: number }[];
  proj: { t: number; v: number; lo: number; hi: number; c: number }[];
  t0: number; t1: number; yMin: number; yMax: number;
}
const sec = (iso: string) => Date.parse(`${iso}T00:00:00Z`) / 1000;
export function fan(history: Point[], points: ProjPoint[], start: string, startNetWorth: string, target: number | null, fromZero = true): Fan {
  const hist = history.map((p) => ({ t: sec(p.date), v: Number(p.net_worth) }));
  const proj = [
    { t: sec(start), v: Number(startNetWorth), lo: Number(startNetWorth), hi: Number(startNetWorth), c: 0 },
    ...points.map((p) => ({ t: sec(p.date), v: Number(p.net_worth), lo: Number(p.low), hi: Number(p.high), c: Number(p.contributed) })),
  ];
  const all = [...hist.map((h) => h.v), ...proj.map((p) => p.hi), ...proj.map((p) => p.lo)];
  if (target != null) all.push(target);
  const hi = Math.max(...all, 1);
  const lo = Math.min(...all);
  if (!fromZero) {
    const pad = (hi - lo) * 0.08 || Math.abs(hi) * 0.05;
    return { hist, proj, t0: hist.length ? hist[0].t : proj[0].t, t1: proj[proj.length - 1].t, yMin: lo - pad, yMax: hi + pad };
  }
  return { hist, proj, t0: hist.length ? hist[0].t : proj[0].t, t1: proj[proj.length - 1].t, yMin: Math.min(0, lo), yMax: hi * 1.04 };
}

/** Round axis steps: 1, 2, 2.5 or 5 × 10^n, aiming for about `n` lines. */
export function niceStep(range: number, n = 5): number {
  if (range <= 0) return 1;
  const raw = range / n;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const f = raw / mag;
  const nice = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10;
  return nice * mag;
}
