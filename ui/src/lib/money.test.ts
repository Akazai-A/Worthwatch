import { describe, expect, it } from "vitest";
import { formatCompact, formatMoney, formatSigned, parseAmount, parseNumber } from "./money";

describe("parseAmount", () => {
  it("handles plain, comma, k, negative and unicode minus", () => {
    expect(parseAmount("5102")).toBe("5102.00");
    expect(parseAmount("5,102.10")).toBe("5102.10");
    expect(parseAmount("18.5k")).toBe("18500.00");
    expect(parseAmount("-640")).toBe("-640.00");
    expect(parseAmount("−640")).toBe("-640.00");
    expect(parseAmount("$1,200")).toBe("1200.00");
    expect(parseAmount("1.2m")).toBe("1200000.00");
  });
  it("rejects junk and empties", () => {
    expect(parseAmount("abc")).toBeNull();
    expect(parseAmount("")).toBeNull();
    expect(parseAmount("1..2")).toBeNull();
  });
});

describe("parseNumber", () => {
  it("keeps unit precision", () => {
    expect(parseNumber("412.5")).toBe("412.5");
    expect(parseNumber("0.123456789")).toBe("0.123457");
  });
});

describe("format", () => {
  it("money uses a real minus sign and no decimals by default", () => {
    expect(formatMoney("1234.56")).toBe("$1,235");
    expect(formatMoney("-640.12")).toBe("−$640");
    expect(formatMoney("640.12", "CAD", 2)).toBe("$640.12");
  });
  it("signed adds a plus only for positives", () => {
    expect(formatSigned("1240")).toBe("+$1,240");
    expect(formatSigned("-5")).toBe("−$5");
    expect(formatSigned("0")).toBe("$0");
  });
  it("compact axis ticks", () => {
    expect(formatCompact(850)).toBe("$850");
    expect(formatCompact(12_300)).toBe("$12.3K");
    expect(formatCompact(284_310)).toBe("$284K");
    expect(formatCompact(1_250_000)).toBe("$1.3M");
    expect(formatCompact(-42_000)).toBe("−$42K");
  });
});
