import { describe, expect, it } from "vitest";
import type { AccountView } from "../api";
import { checkinMonths } from "./cadence";

const base = {
  institution: null, tags: [], group: "cash" as const, liability: false, as_of: null, days_stale: null,
  missing_fx: false, missing_price: false, holdings: [], positions: null, kind: "chequing" as const, currency: "CAD",
  value: "0", native_value: "0",
};

describe("checkinMonths", () => {
  it("marks the months where a typed balance changed, oldest first", () => {
    const chq: AccountView = { ...base, id: "chq", name: "Chequing", as_of: "2026-11-03", history: [
      { date: "2026-08-31", value: "100" }, { date: "2026-09-01", value: "100" }, { date: "2026-09-30", value: "250" }, { date: "2026-11-03", value: "250" },
    ] };
    const m = checkinMonths([chq], "2026-11-03", 4);
    expect(m.map((x) => x.month)).toEqual(["2026-08", "2026-09", "2026-10", "2026-11"]);
    expect(m.map((x) => x.done)).toEqual([false, true, false, true]);
  });
  it("ignores accounts valued from holdings, which move with prices every day", () => {
    const tfsa: AccountView = { ...base, id: "t", name: "TFSA", group: "investments", positions: [{ symbol: "X", units: "1" }],
      history: [{ date: "2026-10-01", value: "1" }, { date: "2026-10-02", value: "2" }] };
    expect(checkinMonths([tfsa], "2026-10-15", 1)).toEqual([{ month: "2026-10", done: false }]);
  });
});
