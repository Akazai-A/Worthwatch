// The Budget view (GET /api/budget) for the month on screen. Refetched when the month
// or the server's version moves (a tap arriving bumps the version).
import { getBudget, type BudgetView } from "../api";

let current = $state<BudgetView | null>(null);
let error = $state<string | null>(null);
let key = "";
let seq = 0;

export const budget = {
  get current() { return current; },
  get error() { return error; },
  async ensure(version: number, month: string | null, force = false) {
    const k = `${version}|${month}`;
    if (k === key && !force) return;
    key = k;
    const mine = ++seq;
    try {
      const v = await getBudget(month ?? undefined);
      if (mine === seq) { current = v; error = null; }
    } catch (e) {
      if (mine === seq) { error = e instanceof Error ? e.message : String(e); key = ""; }
    }
  },
  reload() { key = ""; },
};
