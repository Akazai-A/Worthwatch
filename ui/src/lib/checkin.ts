// Pure logic for the Check-in screen: turn what the user typed into Mutations.
import type { AccountView, Mutation } from "./api";
import { parseAmount, parseNumber } from "./money";

export interface Entry {
  balance?: string; // raw text typed for a balance account
  units?: Record<string, string>; // symbol -> raw text
  prices?: Record<string, string>; // symbol -> raw text
}

export interface CheckinResult {
  mutations: Mutation[];
  /** accounts whose balance or any position changed */
  touched: string[];
  /** symbols whose price was set */
  priced: string[];
  /** input ids ("acct" or "acct:sym:units"/":price") with text that does not parse */
  invalid: string[];
  /** estimated change in net worth, base currency (ignores FX for positions in other currencies) */
  net_change: number;
}

function fx(rates: Record<string, string>, base: string, cur: string): number | null {
  if (cur === base) return 1;
  const r = rates[cur];
  return r == null ? null : Number(r);
}

/** Build the mutation list. Empty inputs carry forward and are not written. */
export function buildCheckin(
  date: string,
  accounts: AccountView[],
  entries: Record<string, Entry>,
  rates: Record<string, string>,
  base: string,
): CheckinResult {
  const mutations: Mutation[] = [];
  const touched: string[] = [];
  const priced = new Set<string>();
  const invalid: string[] = [];
  let net = 0;

  for (const a of accounts) {
    const e = entries[a.id];
    if (!e) continue;
    const rate = fx(rates, base, a.currency) ?? 0;
    const sign = a.liability ? -1 : 1;

    if (a.positions == null) {
      const raw = (e.balance ?? "").trim();
      if (!raw) continue;
      const v = parseAmount(raw);
      if (v == null) {
        invalid.push(a.id);
        continue;
      }
      if (Number(v) === Number(a.native_value)) continue;
      mutations.push({ op: "set_balance", date, account: a.id, value: v });
      touched.push(a.id);
      net += sign * (Number(v) - Number(a.native_value)) * rate;
      continue;
    }

    let changed = false;
    for (const h of a.holdings) {
      const rawUnits = (e.units?.[h.symbol] ?? "").trim();
      const rawPrice = (e.prices?.[h.symbol] ?? "").trim();
      let units = Number(h.units);
      let price = h.price == null ? null : Number(h.price);
      if (rawPrice) {
        const p = parseNumber(rawPrice);
        if (p == null || Number(p) <= 0) {
          invalid.push(`${a.id}:${h.symbol}:price`);
        } else if (Number(p) !== price) {
          mutations.push({ op: "set_price", symbol: h.symbol, date, price: p });
          priced.add(h.symbol);
          price = Number(p);
          changed = true;
        }
      }
      if (rawUnits) {
        const u = parseNumber(rawUnits);
        if (u == null || Number(u) < 0) {
          invalid.push(`${a.id}:${h.symbol}:units`);
        } else if (Number(u) !== units) {
          const diff = Number((Number(u) - units).toFixed(6));
          mutations.push({
            op: "record_position_change",
            change: { date, account: a.id, symbol: h.symbol, units: String(diff), price: rawPrice ? String(price) : null, note: null },
          });
          units = Number(u);
          changed = true;
        }
      }
      const before = Number(h.units) * (h.price == null ? 0 : Number(h.price));
      const after = units * (price ?? 0);
      net += sign * (after - before) * rate;
    }
    if (changed) touched.push(a.id);
  }
  return { mutations, touched, priced: [...priced], invalid, net_change: Number(net.toFixed(2)) };
}

/** "7 accounts updated, 2 prices" */
export function summarize(r: CheckinResult): string {
  const parts: string[] = [];
  if (r.touched.length) parts.push(`${r.touched.length} account${r.touched.length === 1 ? "" : "s"} updated`);
  if (r.priced.length) parts.push(`${r.priced.length} price${r.priced.length === 1 ? "" : "s"}`);
  return parts.length ? parts.join(", ") : "Nothing to save yet";
}

/** Live delta for one balance input, in the account's own currency. */
export function balanceDelta(raw: string, current: string): number | null | "invalid" {
  const t = raw.trim();
  if (!t) return null;
  const v = parseAmount(t);
  if (v == null) return "invalid";
  return Number(v) - Number(current);
}

/** kebab-case id from a display name, unique against existing ids. */
export function slugify(name: string, existing: string[]): string {
  let base = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  if (!base) base = "account";
  let id = base;
  let n = 2;
  while (existing.includes(id)) id = `${base}-${n++}`;
  return id;
}
