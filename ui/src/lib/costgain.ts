// What you put in vs what it grew, per account, from the Portfolio's securities. Pure.
import type { SecurityRow } from "./api";

export interface CostRow { account: string; name: string; value: number; cost: number | null; gain: number | null }

/** One row per account, biggest first. Cost is value minus gain, both in base currency;
 *  an account with any holding lacking a book cost gets no split (cost and gain null). */
export function costByAccount(securities: SecurityRow[]): CostRow[] {
  const rows = new Map<string, CostRow & { partial: boolean }>();
  for (const s of securities) {
    for (const a of s.accounts) {
      const r = rows.get(a.account) ?? { account: a.account, name: a.name, value: 0, cost: 0, gain: 0, partial: false };
      r.value += Number(a.value ?? 0);
      if (a.gain == null || a.value == null) r.partial = true;
      else r.gain = (r.gain ?? 0) + Number(a.gain);
      rows.set(a.account, r);
    }
  }
  return [...rows.values()]
    .map(({ partial, ...r }) => (partial ? { ...r, cost: null, gain: null } : { ...r, cost: r.value - (r.gain ?? 0) }))
    .sort((x, y) => y.value - x.value);
}
