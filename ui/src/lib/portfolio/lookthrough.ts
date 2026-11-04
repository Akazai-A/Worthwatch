// Browser-side wording for the look-through screens. No arithmetic on money here
// beyond layout; every figure comes from the server.
import type { Bucket, Company, FundStatus, Overlap, SharedName, Via } from "../api";
import { formatCompact, formatPct } from "../format/money";

/** "XEQT.TO → XUS.TO" */
export function viaLabel(v: Via): string {
  return v.path.join(" → ");
}

/** "via VTI, VFV.TO and XEQT.TO" from the distinct top-level funds behind a company. */
export function viaSummary(c: Company): string {
  const tops = [...new Set(c.via.map((v) => v.path[0]))];
  if (tops.length === 0) return "";
  if (tops.length === 1) return `via ${tops[0]}`;
  return `via ${tops.slice(0, -1).join(", ")} and ${tops[tops.length - 1]}`;
}

/** "$4.1k", "$920", "$1.2M" for bar rows where a full figure would crowd the label. */
/** A long absolute folder path, shown as its last two parts: `…/data/holdings`. */
export function shortPath(path: string, keep = 2): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  if (parts.length <= keep + 1) return path;
  return "…/" + parts.slice(-keep).join("/");
}

/** Treemap labels: $4.1k, $1.2M, unsigned. */
export function compactMoney(value: string | number): string {
  return formatCompact(Math.abs(Number(value)), "k");
}

/** Treemap blocks: a few families rather than eleven sectors, so labels stay legible. */
export const FAMILY_ORDER = ["Tech & communication", "Financials", "Consumer & health", "Industry, energy & materials", "Bonds & cash", "Other"] as const;
export type Family = (typeof FAMILY_ORDER)[number];
/** CSS token (without var()) carrying each family's hue; bonds, cash and the rest stay neutral. */
export function familyHue(f: Family): string {
  switch (f) {
    case "Tech & communication": return "--f-tech";
    case "Financials": return "--f-fin";
    case "Consumer & health": return "--f-cons";
    case "Industry, energy & materials": return "--f-ind";
    default: return "--f-none";
  }
}
export function familyOf(slot: string): Family {
  switch (slot) {
    case "tech": case "communication": return "Tech & communication";
    case "financials": return "Financials";
    case "consumer": case "health": return "Consumer & health";
    case "industrials": case "resources": return "Industry, energy & materials";
    case "fixed": return "Bonds & cash";
    default: return "Other";
  }
}

/** One line under the headline: "2,913 companies via 3 funds · top 10 = 31.4% · effective holdings ~210". */
export function headlineParts(count: number, funds: FundStatus[], top10: string, effective?: string): string[] {
  const resolved = funds.filter((f) => f.resolved).length;
  const parts = [`${count.toLocaleString("en-CA")} ${count === 1 ? "company" : "companies"} via ${resolved} ${resolved === 1 ? "fund" : "funds"}`];
  if (count > 0) parts.push(`top 10 = ${formatPct(top10)}`);
  if (effective != null) parts.push(`effective holdings ~${effective}`);
  return parts;
}

/** Brass-toned caveats: undisclosed dollars and funds without a file. */
export function caveats(undisclosed: string, total: string, funds: FundStatus[]): string[] {
  const out: string[] = [];
  const u = Number(undisclosed), t = Number(total);
  if (u > 0 && t > 0) out.push(`${((u / t) * 100).toFixed(1)}% undisclosed`);
  const missing = funds.filter((f) => !f.resolved).map((f) => f.symbol);
  if (missing.length === 1) out.push(`${missing[0]} has no holdings file`);
  else if (missing.length > 3) out.push(`${missing.slice(0, 2).join(", ")} and ${missing.length - 2} more have no holdings file`);
  else if (missing.length > 1) out.push(`${missing.join(", ")} have no holdings file`);
  return out;
}

/** Keep the top N buckets and fold the rest, so a bar set stays one screen tall. */
export function topBuckets(b: Bucket[], n: number): Bucket[] {
  if (b.length <= n + 1) return b;
  const head = b.slice(0, n);
  const tail = b.slice(n);
  const value = tail.reduce((s, x) => s + Number(x.value), 0);
  const pct = tail.reduce((s, x) => s + Number(x.pct), 0);
  return [...head, { label: `${tail.length} others`, value: value.toFixed(2), pct: pct.toFixed(1) }];
}

/** What one held fund is made of, by treemap family, as shares of the fund's disclosed dollars.
 *  Layout only: the widths of a stacked bar. Families in FAMILY_ORDER, empty ones dropped. */
export function fundMix(companies: Company[], symbol: string): { family: Family; share: number }[] {
  const by = new Map<Family, number>();
  let sum = 0;
  for (const c of companies) {
    for (const v of c.via) {
      if (v.path[0] !== symbol) continue;
      const f = familyOf(c.slot);
      by.set(f, (by.get(f) ?? 0) + Number(v.value));
      sum += Number(v.value);
    }
  }
  if (sum <= 0) return [];
  return FAMILY_ORDER.filter((f) => (by.get(f) ?? 0) > 0).map((f) => ({ family: f, share: by.get(f)! / sum }));
}

export interface Pair { a: string; b: string; pct: number; shared_value: string; shared: SharedName[] }
/** Each pair of funds with a file once, biggest overlap first. `a` is the side with fewer dollars
 *  in common, so the sentence reads "$X of your A is already in B". */
export function overlapPairs(o: Overlap): Pair[] {
  const idx = o.symbols.map((_, i) => i).filter((i) => o.cells[i][i] != null);
  const out: Pair[] = [];
  for (const i of idx) for (const j of idx) {
    if (j <= i) continue;
    const ij = o.cells[i][j], ji = o.cells[j][i];
    if (!ij || !ji) continue;
    const [a, b, cell] = Number(ij.shared_value) <= Number(ji.shared_value) ? [i, j, ij] : [j, i, ji];
    out.push({ a: o.symbols[a], b: o.symbols[b], pct: Number(ij.pct), shared_value: cell.shared_value, shared: cell.shared });
  }
  return out.sort((x, y) => y.pct - x.pct);
}

/** Two funds this much alike get a flag. */
export const FLAG = { pair: 50 } as const;

/** For each fund, its biggest overlap with another of your funds, if any. */
export function closestTwin(pairs: Pair[], symbol: string): { other: string; pct: number } | null {
  const p = pairs.find((q) => q.a === symbol || q.b === symbol);
  return p ? { other: p.a === symbol ? p.b : p.a, pct: p.pct } : null;
}
