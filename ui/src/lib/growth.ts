// Added vs grown over a window of the growth series: pure, so it is unit-tested.
import type { GrowthPoint } from "./api";

export interface Split { from: string; to: string; start: number; end: number; added: number; grown: number }

/** The split between the first and last point on or after `cut` (ISO date); null with fewer than two. */
export function splitSince(points: GrowthPoint[], cut: string | null): Split | null {
  const i = cut == null ? 0 : Math.max(0, points.findIndex((p) => p.date >= cut) - 1);
  const a = points[i], b = points[points.length - 1];
  if (!a || !b || a === b) return null;
  const start = Number(a.net_worth), end = Number(b.net_worth);
  const added = Number(b.added) - Number(a.added);
  return { from: a.date, to: b.date, start, end, added, grown: end - start - added };
}

/** First date of a chart range ending at `last` (ISO): 3M = 92 days, 1Y = 366, All = null. */
export function rangeCut(range: "3m" | "1y" | "all", last: string): string | null {
  if (range === "all") return null;
  const d = new Date(last + "T00:00:00Z");
  d.setUTCDate(d.getUTCDate() - (range === "3m" ? 92 : 366));
  return d.toISOString().slice(0, 10);
}

export interface MonthBar { month: string; added: number; grown: number }

/** The last `n` calendar months ending at `asOf` (YYYY-MM-DD), oldest first, with empty months as zero. */
export function lastMonths(splits: { month: string; added: string; grown: string }[], asOf: string, n = 12): MonthBar[] {
  const by = new Map(splits.map((m) => [m.month, m]));
  const out: MonthBar[] = [];
  const y = Number(asOf.slice(0, 4)), mo = Number(asOf.slice(5, 7)) - 1;
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date(Date.UTC(y, mo - i, 1));
    const key = d.toISOString().slice(0, 7);
    const m = by.get(key);
    out.push({ month: key, added: Number(m?.added ?? 0), grown: Number(m?.grown ?? 0) });
  }
  return out;
}
