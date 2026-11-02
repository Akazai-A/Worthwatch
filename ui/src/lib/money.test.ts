import { describe, expect, it } from "vitest";
import { parseAmount } from "./money";

describe("parseAmount", () => {
  it("handles plain, comma, k and negative", () => {
    expect(parseAmount("5102")).toBe("5102.00");
    expect(parseAmount("5,102.10")).toBe("5102.10");
    expect(parseAmount("18.5k")).toBe("18500.00");
    expect(parseAmount("-640")).toBe("-640.00");
  });
  it("rejects junk", () => {
    expect(parseAmount("abc")).toBeNull();
  });
});
