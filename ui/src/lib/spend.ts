// Adding a purchase by hand (Budget's + Add, Play's + cash spend): pure, so it is
// tested. The server checks the entry again; this catches mistakes before a round trip.
import type { Account, Spend } from "./api";
import { parseAmount } from "./money";

export interface Draft { amount: string; merchant: string; category: string; account: string; date: string }

/** "" for category means Worthwatch picks with your rules; "" for account means cash. */
export const blankDraft = (today: string): Draft => ({ amount: "", merchant: "", category: "", account: "", date: today });

const PAYS = new Set(["chequing", "savings", "credit_card", "line_of_credit"]);

/** Accounts you pay with: bank accounts and cards, never an investment account. */
export function payable<A extends Pick<Account, "kind" | "positions">>(accounts: A[]): A[] {
  return accounts.filter((a) => PAYS.has(a.kind) && !a.positions);
}

/** The entry to send, or a sentence saying what to fix. */
export function toSpend(d: Draft, today: string): Spend | string {
  const amount = parseAmount(d.amount);
  if (amount == null || Number(amount) <= 0) return "Enter what it cost, such as 12.50.";
  const merchant = d.merchant.trim();
  if (!merchant) return "Say what it was, such as Farmers market.";
  if (merchant.length > 120) return "Keep what it was under 120 characters.";
  if (!/^\d{4}-\d{2}-\d{2}$/.test(d.date)) return "Pick a date.";
  if (d.date > today) return "Pick today or an earlier day.";
  return { date: d.date, merchant, amount, account: d.account || null, category: d.category || null };
}

/** "Farmers market · $12.50" for the confirmation and the chat card's outcome. */
export function savedLine(spends: Pick<Spend, "merchant" | "amount">[], money: (v: string) => string): string {
  if (spends.length === 1) return `${spends[0].merchant} · ${money(spends[0].amount)}`;
  const total = spends.reduce((a, s) => a + Number(s.amount), 0);
  return `${spends.length} purchases · ${money(total.toFixed(2))}`;
}
