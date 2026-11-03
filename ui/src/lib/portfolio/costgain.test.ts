import { describe, expect, it } from "vitest";
import { costByAccount } from "./costgain";
import type { SecurityRow } from "../api";

const sec = (symbol: string, accounts: { account: string; name: string; value: string | null; gain: string | null }[]) =>
  ({ symbol, accounts: accounts.map((a) => ({ ...a, units: "1", book_cost: null })) }) as unknown as SecurityRow;

describe("cost by account", () => {
  it("sums value and gain per account and derives cost in base currency", () => {
    const rows = costByAccount([
      sec("VFV.TO", [{ account: "tfsa", name: "TFSA", value: "100", gain: "30" }]),
      sec("XEQT.TO", [{ account: "tfsa", name: "TFSA", value: "50", gain: "-5" }]),
      sec("VTI", [{ account: "rrsp", name: "RRSP", value: "400", gain: "150" }]),
    ]);
    expect(rows).toEqual([
      { account: "rrsp", name: "RRSP", value: 400, cost: 250, gain: 150 },
      { account: "tfsa", name: "TFSA", value: 150, cost: 125, gain: 25 },
    ]);
  });
  it("leaves the split out when a holding has no book cost", () => {
    const [r] = costByAccount([sec("A", [{ account: "x", name: "X", value: "10", gain: null }])]);
    expect(r.cost).toBeNull();
    expect(r.gain).toBeNull();
  });
});
