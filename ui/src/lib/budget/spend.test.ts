import { describe, expect, it } from "vitest";
import { blankDraft, payable, savedLine, toSpend } from "./spend";
import type { Account } from "../api";

const today = "2026-11-18";
const acct = (id: string, kind: Account["kind"], positions: Account["positions"] = null): Account => ({ id, name: id, kind, currency: "CAD", positions });

describe("adding a purchase by hand", () => {
  it("turns the form into an entry: cash and rules-picked category by default", () => {
    const d = { ...blankDraft(today), amount: "$12.5", merchant: "  Farmers market " };
    expect(toSpend(d, today)).toEqual({ date: today, merchant: "Farmers market", amount: "12.50", account: null, category: null });
    expect(toSpend({ ...d, account: "td-visa", category: "groceries" }, today)).toMatchObject({ account: "td-visa", category: "groceries" });
  });

  it("says what to fix instead of sending a bad entry", () => {
    const d = { ...blankDraft(today), amount: "12", merchant: "Lunch" };
    expect(toSpend({ ...d, amount: "" }, today)).toMatch(/what it cost/);
    expect(toSpend({ ...d, amount: "-4" }, today)).toMatch(/what it cost/);
    expect(toSpend({ ...d, merchant: " " }, today)).toMatch(/what it was/);
    expect(toSpend({ ...d, date: "2026-11-19" }, today)).toMatch(/today or an earlier/);
    expect(typeof toSpend({ ...d, date: "2026-10-31" }, today)).toBe("object");
  });

  it("offers bank accounts and cards to pay with, never an investment account", () => {
    const ids = payable([acct("chq", "chequing"), acct("visa", "credit_card"), acct("tfsa", "tfsa"), acct("loc", "line_of_credit"), acct("cash-in-tfsa", "savings", [{ symbol: "XEQT", units: "1" }])]).map((a) => a.id);
    expect(ids).toEqual(["chq", "visa", "loc"]);
  });

  it("confirms one entry by name and several by count", () => {
    const m = (v: string) => `$${Number(v).toFixed(2)}`;
    expect(savedLine([{ merchant: "Coffee", amount: "6" }], m)).toBe("Coffee · $6.00");
    expect(savedLine([{ merchant: "Coffee", amount: "6" }, { merchant: "Lunch", amount: "14.5" }], m)).toBe("2 purchases · $20.50");
  });
});
