// Thin client. Types mirror server/src/routes.rs for now; Stage 1 generates them.
export type Group = "cash" | "investments" | "property" | "debt";

export interface AccountView {
  id: string;
  name: string;
  institution?: string;
  kind: string;
  currency: string;
  tags?: string[];
  group: Group;
  value: string; // Decimal serialized as string
  days_stale: number | null;
  missing_fx: boolean;
  missing_price: boolean;
}

export interface Point { date: string; net_worth: string; assets: string; liabilities: string }

export interface LedgerView {
  as_of: string;
  base_currency: string;
  totals: { assets: string; liabilities: string; net_worth: string; by_group: Record<Group, string> };
  series: Point[];
  accounts: AccountView[];
}

export async function getView(): Promise<LedgerView> {
  const r = await fetch("/api/view");
  if (!r.ok) throw new Error(`view: ${r.status}`);
  return r.json();
}

export async function apply(mutations: unknown[]): Promise<LedgerView> {
  const r = await fetch("/api/apply", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(mutations),
  });
  if (!r.ok) {
    const e = await r.json().catch(() => ({ message: r.statusText }));
    throw new Error(e.line ? `line ${e.line}: ${e.message}` : e.message);
  }
  return r.json();
}
