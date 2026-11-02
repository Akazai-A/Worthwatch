import { describe, expect, it } from "vitest";
import type { AccountView } from "./api";
import { balanceDelta, buildCheckin, slugify, summarize } from "./checkin";

const base = {
  institution: null, tags: [], group: "cash" as const, liability: false, as_of: null, days_stale: null,
  missing_fx: false, missing_price: false, history: [], holdings: [], positions: null,
};
const chq: AccountView = { ...base, id: "chq", name: "Chequing", kind: "chequing", currency: "CAD", value: "5102.10", native_value: "5102.10" };
const visa: AccountView = { ...base, id: "visa", name: "Visa", kind: "credit_card", currency: "CAD", group: "debt", liability: true, value: "640", native_value: "640" };
const usd: AccountView = { ...base, id: "usd", name: "USD", kind: "chequing", currency: "USD", value: "1380", native_value: "1000" };
const tfsa: AccountView = {
  ...base, id: "tfsa", name: "TFSA", kind: "tfsa", currency: "CAD", group: "investments", value: "14074.5", native_value: "14074.5",
  positions: [{ symbol: "XEQT.TO", units: "412.5" }],
  holdings: [{ symbol: "XEQT.TO", units: "412.5", price: "34.12", price_date: "2026-10-31", price_source: "manual", native_value: "14074.5", value: "14074.5", book_cost: null }],
};
const rates = { USD: "1.38" };

describe("buildCheckin", () => {
  it("writes only what changed and carries the rest forward", () => {
    const r = buildCheckin("2026-11-02", [chq, visa, usd], { chq: { balance: "6,240" }, visa: { balance: "" }, usd: { balance: "1000" } }, rates, "CAD");
    expect(r.mutations).toEqual([{ op: "set_balance", date: "2026-11-02", account: "chq", value: "6240.00" }]);
    expect(r.touched).toEqual(["chq"]);
    expect(r.net_change).toBeCloseTo(1137.9, 2);
    expect(summarize(r)).toBe("1 account updated");
  });

  it("debt growing lowers net worth and fx is applied", () => {
    const r = buildCheckin("2026-11-02", [visa, usd], { visa: { balance: "1000" }, usd: { balance: "1100" } }, rates, "CAD");
    expect(r.net_change).toBeCloseTo(-360 + 138, 2);
  });

  it("flags unparseable input and skips it", () => {
    const r = buildCheckin("2026-11-02", [chq], { chq: { balance: "lots" } }, rates, "CAD");
    expect(r.invalid).toEqual(["chq"]);
    expect(r.mutations).toEqual([]);
  });

  it("positions: units change logs a trade, price change sets a price", () => {
    const r = buildCheckin(
      "2026-11-02", [tfsa],
      { tfsa: { units: { "XEQT.TO": "426.5" }, prices: { "XEQT.TO": "34.90" } } }, rates, "CAD",
    );
    expect(r.mutations).toEqual([
      { op: "set_price", symbol: "XEQT.TO", date: "2026-11-02", price: "34.9" },
      { op: "record_position_change", change: { date: "2026-11-02", account: "tfsa", symbol: "XEQT.TO", units: "14", price: "34.9", note: null } },
    ]);
    expect(r.priced).toEqual(["XEQT.TO"]);
    expect(r.touched).toEqual(["tfsa"]);
    expect(r.net_change).toBeCloseTo(426.5 * 34.9 - 412.5 * 34.12, 2);
    expect(summarize(r)).toBe("1 account updated, 1 price");
  });

  it("empty everything saves nothing", () => {
    const r = buildCheckin("2026-11-02", [chq, tfsa], {}, rates, "CAD");
    expect(r.mutations).toEqual([]);
    expect(summarize(r)).toBe("Nothing to save yet");
  });
});

describe("helpers", () => {
  it("balanceDelta", () => {
    expect(balanceDelta("", "5")).toBeNull();
    expect(balanceDelta("x", "5")).toBe("invalid");
    expect(balanceDelta("7.5", "5")).toBe(2.5);
  });
  it("slugify makes unique kebab ids", () => {
    expect(slugify("TD Chequing!", [])).toBe("td-chequing");
    expect(slugify("TD Chequing", ["td-chequing"])).toBe("td-chequing-2");
    expect(slugify("***", [])).toBe("account");
  });
});
