// The check-in rhythm: which of the last 12 months had a balance entered by hand.
// A typed balance shows up as a step in an account's daily history; accounts valued from
// holdings move with prices every day, so only balance accounts count.
import type { AccountView } from "../api";

export interface CadenceMonth { month: string; done: boolean }

export function checkinMonths(accounts: AccountView[], asOf: string, count = 12): CadenceMonth[] {
  const done = new Set<string>();
  for (const a of accounts) {
    if (a.positions != null) continue;
    const h = a.history;
    for (let i = 1; i < h.length; i++) if (h[i].value !== h[i - 1].value) done.add(h[i].date.slice(0, 7));
    if (a.as_of) done.add(a.as_of.slice(0, 7));
  }
  const y = Number(asOf.slice(0, 4)), m = Number(asOf.slice(5, 7)) - 1;
  const out: CadenceMonth[] = [];
  for (let k = count - 1; k >= 0; k--) {
    const d = new Date(Date.UTC(y, m - k, 1));
    const month = `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
    out.push({ month, done: done.has(month) });
  }
  return out;
}
