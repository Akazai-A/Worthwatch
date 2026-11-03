// A holding's page: which quote symbol to ask for, what a fund holds, and the price figures
// over a range. Layout arithmetic only; dollars come from the server.
import type { Company } from "../api";

/** The quote symbol for a company inside a fund: Canadian listings carry `.TO`. */
export function quoteSymbol(c: Pick<Company, "ticker" | "currency" | "name">): string | null {
  const t = (c.ticker ?? "").trim().toUpperCase();
  if (!t || t === "-" || !/^[A-Z0-9.\-]{1,12}$/.test(t)) return null;
  if (t.includes(".")) return t.replace(/\.(?=[A-Z]$)/, "-"); // BRK.B -> BRK-B
  return c.currency === "CAD" ? `${t}.TO` : t;
}

/** What one fund holds, through funds inside it: each company's share of your dollars in that fund. */
export function insideOf(companies: Company[], symbol: string, n = 12): { c: Company; value: number; share: number }[] {
  const rows: { c: Company; value: number }[] = [];
  let total = 0;
  for (const c of companies) {
    if (c.undisclosed || c.key === "__other__") continue;
    const v = c.via.filter((x) => x.path.includes(symbol)).reduce((s, x) => s + Number(x.value), 0);
    if (v > 0) { rows.push({ c, value: v }); total += v; }
  }
  return rows.sort((a, b) => b.value - a.value).slice(0, n).map((r) => ({ ...r, share: total > 0 ? (r.value / total) * 100 : 0 }));
}

export const RANGES = [
  { id: "1m", label: "1M", days: 31 },
  { id: "3m", label: "3M", days: 92 },
  { id: "6m", label: "6M", days: 183 },
  { id: "1y", label: "1Y", days: 366 },
] as const;
export type Range = (typeof RANGES)[number]["id"];

/** The closes inside a range ending at the last close, with the move and the high and low. */
export function window(prices: { date: string; close: string }[], range: Range) {
  if (prices.length === 0) return null;
  const days = RANGES.find((r) => r.id === range)!.days;
  const end = new Date(prices[prices.length - 1].date).getTime();
  const pts = prices.filter((p) => end - new Date(p.date).getTime() <= days * 86400000).map((p) => ({ date: p.date, v: Number(p.close) }));
  if (pts.length === 0) return null;
  const first = pts[0].v, last = pts[pts.length - 1].v;
  const vs = pts.map((p) => p.v);
  return { pts, first, last, change: last - first, pct: first > 0 ? ((last - first) / first) * 100 : null, high: Math.max(...vs), low: Math.min(...vs) };
}
