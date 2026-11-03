// Browser-side wording and layout for the Changes screen. Period arithmetic on
// dates only; every dollar figure comes from the server.
import type { ConstituentChange, FundFeed, HoldingDiff } from "../api";
import { formatDate, formatPct } from "../format/money";

export type Preset = "checkin" | "1m" | "3m" | "ytd" | "1y" | "custom";
export const PRESETS: { id: Preset; label: string; title: string }[] = [
  { id: "checkin", label: "Since last check-in", title: "From the check-in before the end date" },
  { id: "1m", label: "1M", title: "Last month" },
  { id: "3m", label: "3M", title: "Last three months" },
  { id: "ytd", label: "YTD", title: "Since January 1" },
  { id: "1y", label: "1Y", title: "Last year" },
  { id: "custom", label: "Custom", title: "Pick both dates" },
];

function iso(d: Date): string {
  return d.toISOString().slice(0, 10);
}
function utc(s: string): Date {
  return new Date(`${s}T00:00:00Z`);
}
function minusMonths(s: string, n: number): string {
  const d = utc(s);
  const day = d.getUTCDate();
  d.setUTCDate(1);
  d.setUTCMonth(d.getUTCMonth() - n);
  const last = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 0)).getUTCDate();
  d.setUTCDate(Math.min(day, last));
  return iso(d);
}

/** Start date for a preset, clamped to the first date anything is known. `undefined` lets the server pick. */
export function presetFrom(p: Preset, today: string, firstDate: string | null, custom: { from: string; to: string } | null): string | undefined {
  let from: string | undefined;
  switch (p) {
    case "checkin": return undefined;
    case "1m": from = minusMonths(today, 1); break;
    case "3m": from = minusMonths(today, 3); break;
    case "1y": from = minusMonths(today, 12); break;
    case "ytd": from = `${today.slice(0, 4)}-01-01`; break;
    case "custom": from = custom?.from; break;
  }
  if (from && firstDate && from < firstDate) from = firstDate;
  return from;
}

export function presetTo(p: Preset, today: string, custom: { from: string; to: string } | null): string | undefined {
  return p === "custom" ? custom?.to : undefined;
}

/** "Sep 30 → Nov 2, 2026 · 33 days" */
export function rangeLabel(from: string, to: string, days: number): string {
  const sameYear = from.slice(0, 4) === to.slice(0, 4);
  return `${formatDate(from, sameYear ? "short" : "long")} → ${formatDate(to, "long")} · ${days} ${days === 1 ? "day" : "days"}`;
}

export interface Part { label: string; value: number; hint: string }

/** The headline split, leaving out parts that are zero. */
export function parts(c: { market: string; trades: string; fx: string; balances: string; debt: string; residual: string }): Part[] {
  const all: Part[] = [
    { label: "market", value: Number(c.market), hint: "Prices moved on units you already held" },
    { label: "contributions & trades", value: Number(c.trades), hint: "Units bought or sold, valued at the end price" },
    { label: "cash & property", value: Number(c.balances), hint: "Balances you typed moved" },
    { label: Number(c.debt) >= 0 ? "debt paid down" : "debt grew", value: Number(c.debt), hint: "Liabilities moved; paying down counts as a gain" },
    { label: "FX", value: Number(c.fx), hint: "Exchange rates moved on the starting value" },
    { label: "unpriced", value: Number(c.residual), hint: "Holdings without a price at one end, or no FX rate" },
  ];
  return all.filter((p) => Math.abs(p.value) >= 0.005);
}

/** What mostly moved a waterfall step: prices and FX, money you moved in or out, or a debt. */
export type Cause = "market" | "money" | "debt";
export interface BridgeStep { label: string; delta: number; folded?: number; cause?: Cause }

/** One sentence for the top of Changes: the fund that did most of it, or nothing when it was spread out. */
export function verdict(steps: BridgeStep[], change: number): { who: string; delta: number } | null {
  const top = steps.filter((s) => !s.folded).sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta))[0];
  if (!top || Math.abs(change) < 0.5) return null;
  return Math.abs(top.delta) >= Math.abs(change) * 0.25 ? { who: top.label, delta: top.delta } : null;
}

/** One waterfall step per fund that moved, biggest first; coloured by what moved it most. */
export function fundSteps(holdings: HoldingDiff[]): BridgeStep[] {
  return holdings
    .filter((h) => Math.abs(Number(h.change ?? 0)) >= 0.005)
    .map((h) => {
      const m = Math.abs(Number(h.market ?? 0)) + Math.abs(Number(h.fx ?? 0));
      return { label: h.symbol, delta: Number(h.change), cause: (m >= Math.abs(Number(h.trades ?? 0)) ? "market" : "money") as Cause };
    })
    .sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta));
}

/** The split of the investment change: price, money you put in or took out, FX. */
export function investParts(holdings: HoldingDiff[]): Part[] {
  const sum = (k: "market" | "trades" | "fx") => holdings.reduce((s, h) => s + Number(h[k] ?? 0), 0);
  return [
    { label: "market", value: sum("market"), hint: "the price move on units you already held" },
    { label: "you bought or sold", value: sum("trades"), hint: "units bought or sold, at the end price" },
    { label: "FX", value: sum("fx"), hint: "the exchange-rate move on what you held in other currencies" },
  ].filter((p) => Math.abs(p.value) >= 0.005);
}

/** "XEC 7.37% → 7.92% (+0.55 pt)" / "Celestica added at 0.84%" / "Algonquin Power removed (was 0.11%)". */
export function changeLine(c: ConstituentChange): string {
  const who = c.ticker && c.ticker !== "-" ? c.ticker : c.name;
  const pts = `${Number(c.delta) > 0 ? "+" : "−"}${Math.abs(Number(c.delta)).toFixed(2)} pt`;
  switch (c.kind) {
    case "added": return `${who} added at ${formatPct(c.to_weight, 2)}`;
    case "removed": return `${who} removed (was ${formatPct(c.from_weight, 2)})`;
    default: return `${who} ${formatPct(c.from_weight, 2)} → ${formatPct(c.to_weight, 2)} (${pts})`;
  }
}

/** The second line of a feed item: what else moved, under the threshold. */
export function minorLine(f: FundFeed): string {
  const bits: string[] = [];
  if (f.minor_added) bits.push(`${f.minor_added} added`);
  if (f.minor_removed) bits.push(`${f.minor_removed} removed`);
  if (f.minor_reweighted) bits.push(`${f.minor_reweighted} reweighted`);
  const under = bits.length ? `${bits.join(", ")} under ${Number(f.threshold).toFixed(1)} pt` : "";
  const names = `${f.to_constituents.toLocaleString("en-CA")} names`;
  const turnover = Number(f.turnover_pct) > 0 ? `${formatPct(f.turnover_pct)} of the fund changed hands` : "";
  return [names, under, turnover].filter(Boolean).join(" · ");
}
