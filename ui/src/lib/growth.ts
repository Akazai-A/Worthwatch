// Added vs grown: pure, so it is unit-tested. The Home chart and the account page use these.
import type { GrowthPoint } from "./api";

/** Where net worth would be on what you put in alone, from the window's first point.
 *  The gap between this line and net worth is what the market and FX did. */
export function addedLine(visible: { date: string; net_worth: string }[], growth: GrowthPoint[]): number[] {
  if (!visible.length) return [];
  const by = new Map(growth.map((p) => [p.date, Number(p.added)]));
  const a0 = by.get(visible[0].date) ?? 0;
  const n0 = Number(visible[0].net_worth);
  let last = a0;
  return visible.map((p) => { last = by.get(p.date) ?? last; return n0 + last - a0; });
}

export interface WindowTotals { from: string; added: number; grown: number; fromYou: number | null }

/** The window's totals: what you added, what the market did, and your share when both helped. */
export function windowTotals(visible: { date: string; net_worth: string }[], line: number[]): WindowTotals | null {
  if (visible.length < 2 || line.length !== visible.length) return null;
  const start = Number(visible[0].net_worth);
  const end = Number(visible[visible.length - 1].net_worth);
  const atEnd = line[line.length - 1];
  const added = atEnd - start, grown = end - atEnd;
  const fromYou = added > 0 && grown > 0 ? Math.round((added / (added + grown)) * 100) : null;
  return { from: visible[0].date, added, grown, fromYou };
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
