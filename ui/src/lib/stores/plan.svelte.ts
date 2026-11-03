// The Plan view (GET /api/plan). Slider values live here, not in the ledger, until
// the user saves them as their defaults. Refetched when the inputs or the ledger move.
import { getPlan, type PlanView } from "../api";

let current = $state<PlanView | null>(null);
let error = $state<string | null>(null);
let years = $state(20);
let returnPct = $state<string | null>(null);
let extra = $state("0");
let target = $state<string | null>(null);
let key = "";
let seq = 0;

export const plan = {
  get current() { return current; },
  get error() { return error; },
  get years() { return years; },
  get returnPct() { return returnPct; },
  get extra() { return extra; },
  get target() { return target; },
  setYears(y: number) { years = y; },
  setReturn(r: string | null) { returnPct = r; },
  setExtra(x: string) { extra = x; },
  setTarget(t: string | null) { target = t; },
  /** Back to the ledger's own assumptions. */
  reset() { returnPct = null; extra = "0"; target = null; },
  async ensure(version: number) {
    const k = `${version}|${years}|${returnPct}|${extra}|${target}`;
    if (k === key) return;
    key = k;
    const mine = ++seq;
    try {
      const v = await getPlan({
        years,
        return_pct: returnPct ?? undefined,
        extra: extra !== "0" ? extra : undefined,
        target: target ?? undefined,
      });
      // Sliders fire fast; only the latest answer counts.
      if (mine === seq) { current = v; error = null; }
    } catch (e) {
      if (mine === seq) { error = e instanceof Error ? e.message : String(e); key = ""; }
    }
  },
};
