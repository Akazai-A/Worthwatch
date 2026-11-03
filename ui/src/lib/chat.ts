// Pure logic for the Chat screen: labels for diff rows, the edit step, and the
// provider line. The server builds and checks every plan; this only reshapes one
// when the user corrects a proposed figure, and the server checks it again on apply.
import type { AssistantView, Mutation, Plan, PlanRow2, Spend } from "./api";
import { formatUnits, parseNumber } from "./money";

/** The faint second line under a row's name. */
export function rowHint(r: PlanRow2): string {
  const parts: string[] = [];
  if (r.kind === "trade" && r.delta != null) {
    const n = Number(r.delta);
    parts.push(`${n < 0 ? "sell" : "buy"} ${fmtUnits(Math.abs(n))}${r.price ? ` @ ${Number(r.price).toFixed(2)}` : " · no price yet"}`);
  }
  if (r.kind === "price") parts.push(`price in ${r.currency}`);
  if (r.kind === "new_account" && r.detail) parts.push(r.detail);
  if (r.source) parts.push(`from “${r.source}”`);
  return parts.join(" · ");
}

export function rowTitle(r: PlanRow2): string {
  if (r.kind === "trade" || r.kind === "price") return r.kind === "price" ? `${r.detail} price` : `${r.account_name} · ${r.detail}`;
  if (r.kind === "note") return "Note";
  if (r.kind === "spend") return r.detail ?? "Purchase";
  return r.account_name;
}

/** The purchase behind spend row `i`: spend rows and `plan.spends` are in the same order. */
export function spendOf(plan: Plan, i: number): Spend | null {
  if (plan.rows[i]?.kind !== "spend") return null;
  const k = plan.rows.slice(0, i).filter((r) => r.kind === "spend").length;
  return plan.spends?.[k] ?? null;
}

/** Only purchases: the card talks about the budget, not net worth. */
export const onlySpends = (plan: Plan) => plan.rows.length > 0 && plan.rows.every((r) => r.kind === "spend");

/** Something on the card can be saved: a ledger change or a purchase. */
export const hasWork = (plan: Plan) => plan.mutations.length > 0 || (plan.spends?.length ?? 0) > 0;

export function fmtUnits(n: number): string {
  return formatUnits(n);
}

/** Can the "to" figure be edited in place? Balances, trades and prices. */
export function editable(r: PlanRow2): boolean {
  return r.kind === "balance" || r.kind === "trade" || r.kind === "price";
}

/**
 * Apply the user's corrections (row index -> new "to" text) to a plan. Rewrites the
 * matching mutation and the row's delta and effect. Returns null when any edit does
 * not parse, so the card can mark it. Units sold below zero are left for the server
 * to refuse on apply, which it does with a message.
 */
export function editPlan(plan: Plan, edits: Record<number, string>, rates: Record<string, string>, base: string): Plan | null {
  const rows = plan.rows.map((r) => ({ ...r }));
  let mutations: Mutation[] = plan.mutations.map((m) => JSON.parse(JSON.stringify(m)) as Mutation);
  for (const [k, text] of Object.entries(edits)) {
    const i = Number(k);
    const r = rows[i];
    if (!r || !text.trim()) continue;
    const v = parseNumber(text);
    if (v == null || Number(v) < 0) return null;
    const to = Number(v);
    const rate = r.currency === base ? 1 : Number(rates[r.currency] ?? NaN);
    if (r.kind === "balance") {
      const from = r.from == null ? null : Number(r.from);
      const exists = mutations.some((m) => m.op === "set_balance" && m.account === r.account);
      mutations = mutations.filter((m) => !(m.op === "set_balance" && m.account === r.account));
      if (from == null || to !== from) mutations.push({ op: "set_balance", date: plan.date, account: r.account!, value: v });
      else if (!exists) { /* unchanged and was unchanged */ }
      r.to = v;
      r.delta = from == null ? null : String(round(to - from, 2));
      r.effect = from == null || Number.isNaN(rate) ? null : String(round((to - from) * (r.liability ? -1 : 1) * rate, 2));
    } else if (r.kind === "trade") {
      const from = r.from == null ? 0 : Number(r.from);
      const units = round(to - from, 6);
      mutations = mutations.map((m) =>
        m.op === "record_position_change" && m.change.account === r.account && m.change.symbol === r.detail
          ? { ...m, change: { ...m.change, units: String(units) } }
          : m,
      );
      r.to = v;
      r.delta = String(units);
      r.effect = r.price == null || Number.isNaN(rate) ? null : String(round(units * Number(r.price) * rate, 2));
    } else if (r.kind === "price") {
      mutations = mutations.map((m) => (m.op === "set_price" && m.symbol === r.detail ? { ...m, price: v } : m));
      const from = r.from == null ? null : Number(r.from);
      const old = Number(r.to);
      const effect = r.effect == null || from == null ? null : Number(r.effect) * (old === from ? 0 : (to - from) / (old - from));
      r.to = v;
      r.delta = from == null ? null : String(round(to - from, 6));
      r.effect = effect == null || Number.isNaN(effect) ? null : String(round(effect, 2));
    }
  }
  const effects = rows.map((r) => (r.effect == null ? null : Number(r.effect)));
  const net = effects.some((e) => e == null) ? null : effects.reduce<number>((a, e) => a + (e ?? 0), 0);
  const before = plan.net_worth_after != null && plan.net_change != null ? Number(plan.net_worth_after) - Number(plan.net_change) : null;
  return {
    ...plan,
    rows,
    mutations,
    net_change: net == null ? null : String(round(net, 2)),
    net_worth_after: net == null || before == null ? null : String(round(before + net, 2)),
  };
}

function round(n: number, dp: number): number {
  const f = 10 ** dp;
  return Math.round(n * f) / f;
}

/** "Ollama · llama3.1 · local, nothing leaves this machine" */
export function providerLine(a: AssistantView): { dot: "local" | "cloud" | "off"; text: string } {
  if (a.offline) return { dot: "off", text: "Offline · built-in parser, no model" };
  switch (a.provider) {
    case "ollama":
      return { dot: "local", text: `Ollama · ${a.model ?? "default model"} · local, nothing leaves this machine` };
    case "cloud":
      return { dot: "cloud", text: `Claude · ${a.model ?? ""} · cloud, opt-in, you see what is sent${a.key_present ? "" : " · ANTHROPIC_API_KEY not set"}` };
    default:
      return { dot: "local", text: "Built-in parser · no model, nothing leaves this machine" };
  }
}

export const EXAMPLES = ["td chq 5102, visa 640", "sold 20 XEQT in the TFSA", "coffee 6 cash", "what is my net worth?"];
