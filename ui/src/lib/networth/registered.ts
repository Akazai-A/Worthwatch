// Browser-side layout for the Registered accounts section of the Plan screen. Every
// figure comes from the server; the setup preview reuses CRA's limits the server sends,
// so you see what your two answers mean before you save them.
import type { CraLimits, RegisteredView, RoomKind, Standing } from "../api";

/** The value bar: put in, growth (or loss), as percentages of the larger of value and put in. */
export interface ValueBar { putIn: number; growth: number; loss: number }
export function valueBar(r: RegisteredView): ValueBar | null {
  if (r.put_in == null) return null;
  const put = Math.max(0, Number(r.put_in));
  const value = Math.max(0, Number(r.value));
  const scale = Math.max(put, value, 1);
  const pct = (v: number) => (v / scale) * 100;
  return { putIn: pct(Math.min(put, value)), growth: pct(Math.max(0, value - put)), loss: pct(Math.max(0, put - value)) };
}

export type Tone = "done" | "room" | "over";
/** The dot and words beside the room figure: "Maxed out", "Maxed for 2026", "Room left", "Over". */
export function roomStatus(r: RegisteredView, year: number): { tone: Tone; text: string } | null {
  const room = r.room;
  if (!room) return null;
  const left = Number(room.left);
  if (room.over) return { tone: "over", text: "Over the limit" };
  if (left > 0) return { tone: "room", text: r.kind === "fhsa" ? `Room in ${year}` : "Room left" };
  if (r.kind === "fhsa") return r.lifetime_left != null && Number(r.lifetime_left) <= 0 ? { tone: "done", text: "Lifetime limit reached" } : { tone: "done", text: `Maxed for ${year}` };
  return { tone: "done", text: "Maxed out" };
}

/** TFSA year columns: height of each year's limit, and how much of it is used, both 0..1. */
export interface Column { year: number; h: number; fill: number; future: boolean; estimated: boolean; limit: number; used: number }
export function columns(r: RegisteredView): Column[] {
  const max = Math.max(1, ...r.years.map((y) => Number(y.limit)));
  return r.years.map((y) => {
    const limit = Number(y.limit);
    const used = Number(y.used);
    return { year: y.year, h: limit / max, fill: limit > 0 ? Math.min(1, used / limit) : 0, future: y.future, estimated: y.estimated, limit, used };
  });
}

/** Years to label under the TFSA strip: the first, 2015 (the $10,000 year) when shown, and this year. */
export function stripLabels(r: RegisteredView, year: number): number[] {
  const ys = r.years.map((y) => y.year).filter((y) => y <= year);
  if (!ys.length) return [];
  const out = [ys[0]];
  if (ys.includes(2015) && ys[0] < 2013 && year > 2017) out.push(2015);
  if (ys[ys.length - 1] !== ys[0]) out.push(ys[ys.length - 1]);
  return out;
}

/** The limit next January 1 adds, from the strip's future column. */
export function nextYearAdds(r: RegisteredView): number | null {
  const f = r.years.find((y) => y.future);
  if (r.kind === "tfsa") return f ? Number(f.limit) : null;
  return null;
}

/** Largest account = 100%; the rest relative to it. */
export function accountBars(r: RegisteredView): { name: string; value: number; pct: number }[] {
  const max = Math.max(1, ...r.accounts.map((a) => Number(a.value)));
  return r.accounts.map((a) => ({ name: a.name, value: Number(a.value), pct: (Math.max(0, Number(a.value)) / max) * 100 }));
}

/** What the setup answers mean, worked out with CRA's limits: all room since the start year,
 *  what is left now, and the total put in. `error` says why an answer cannot be right. */
export interface Preview { since: number; lifetime: number; left: number; putIn: number; error: string | null }
export function preview(kind: RoomKind, since: number, standing: Standing, year: number, l: CraLimits): Preview {
  const first = kind === "tfsa" ? l.tfsa_first_year : l.fhsa_first_year;
  const s = kind === "tfsa" ? Math.max(since, first) : since;
  const bad = (error: string): Preview => ({ since: s, lifetime: 0, left: 0, putIn: 0, error });
  if (!Number.isFinite(s) || s < first || s > year) return bad(`Pick a year from ${first} to ${year}.`);
  const lifetime = kind === "tfsa" ? tfsaLifetime(s, year, l) : fhsaLifetime(s, year, l);
  if (standing.kind === "maxed") return { since: s, lifetime, left: 0, putIn: lifetime, error: null };
  const amount = Number(standing.amount);
  if (!Number.isFinite(amount) || amount < 0) return bad("Enter a dollar amount, 0 or more.");
  if (standing.kind === "left") return { since: s, lifetime, left: amount, putIn: Math.max(0, lifetime - amount), error: null };
  if (amount > lifetime) return bad(`That is more than the ${lifetime.toLocaleString("en-CA", { style: "currency", currency: "CAD", maximumFractionDigits: 0 })} of room since ${s}.`);
  const left = kind === "tfsa" ? lifetime - amount : fhsaLeft(s, amount, year, l);
  return { since: s, lifetime, left, putIn: amount, error: null };
}

export function tfsaLifetime(since: number, year: number, l: CraLimits): number {
  const last = l.tfsa[l.tfsa.length - 1];
  let sum = 0;
  for (let y = Math.max(since, l.tfsa_first_year); y <= year; y++) {
    const hit = l.tfsa.find(([yy]) => yy === y);
    sum += Number(hit ? hit[1] : last?.[1] ?? 0);
  }
  return sum;
}

export function fhsaLifetime(opened: number, year: number, l: CraLimits): number {
  return Math.min(Number(l.fhsa_annual) * Math.max(0, year - opened + 1), Number(l.fhsa_lifetime));
}

/** FHSA room left in `year` if `putIn` went in as early as the rules allowed (same as the server). */
export function fhsaLeft(opened: number, putIn: number, year: number, l: CraLimits): number {
  let unplaced = Math.max(0, putIn), placed = 0, unused = 0;
  for (let y = opened; y <= year; y++) {
    const carry = Math.min(Math.max(0, unused), Number(l.fhsa_carry));
    const room = Math.min(Number(l.fhsa_annual) + carry, Math.max(0, Number(l.fhsa_lifetime) - placed));
    const c = Math.min(unplaced, room);
    unplaced -= c;
    placed += c;
    unused = room - c;
  }
  return unused;
}
