export function formatMoney(value: string | number, currency = "CAD"): string {
  const n = typeof value === "string" ? Number(value) : value;
  return new Intl.NumberFormat("en-CA", { style: "currency", currency, maximumFractionDigits: 0 }).format(n);
}

/** Accepts "5102", "5,102.10", "18.5k", "-640". Returns a decimal string or null. */
export function parseAmount(input: string): string | null {
  const s = input.trim().replace(/[,$\s]/g, "").toLowerCase();
  const m = /^(-?\d+(?:\.\d+)?)(k|m)?$/.exec(s);
  if (!m) return null;
  const mult = m[2] === "k" ? 1000 : m[2] === "m" ? 1_000_000 : 1;
  return (Number(m[1]) * mult).toFixed(2);
}
