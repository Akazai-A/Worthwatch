// Browser-side wording for the look-through screens. No arithmetic on money here
// beyond layout; every figure comes from the server.
import type { Bucket, Company, FundStatus, Via } from "./api";

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

export function compactMoney(value: string | number): string {
  const n = Math.abs(Number(value));
  if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1).replace(/\.0$/, "")}M`;
  if (n >= 1_000) return `$${(n / 1_000).toFixed(n >= 100_000 ? 0 : 1).replace(/\.0$/, "")}k`;
  return `$${Math.round(n)}`;
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
  if (count > 0) parts.push(`top 10 = ${Number(top10).toFixed(1)}%`);
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
