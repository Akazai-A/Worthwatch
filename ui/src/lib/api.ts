// Thin client. Types mirror server/src/routes.rs (Decimal and dates come as strings).
export type Group = "cash" | "investments" | "property" | "debt";
export const GROUPS: Group[] = ["cash", "investments", "property", "debt"];
export const GROUP_LABEL: Record<Group, string> = {
  cash: "Cash",
  investments: "Investments",
  property: "Property",
  debt: "Debt",
};

export type AccountKind =
  | "chequing" | "savings" | "tfsa" | "rrsp" | "fhsa" | "resp" | "non_registered" | "crypto"
  | "property" | "vehicle" | "other_asset"
  | "credit_card" | "line_of_credit" | "mortgage" | "loan" | "other_liability";

export const KIND_LABEL: Record<AccountKind, string> = {
  chequing: "Chequing", savings: "Savings", tfsa: "TFSA", rrsp: "RRSP", fhsa: "FHSA", resp: "RESP",
  non_registered: "Non-registered", crypto: "Crypto", property: "Property", vehicle: "Vehicle",
  other_asset: "Other asset", credit_card: "Credit card", line_of_credit: "Line of credit",
  mortgage: "Mortgage", loan: "Loan", other_liability: "Other liability",
};

export interface Diagnostic { line: number | null; message: string }
export interface Delta { from: string; amount: string; pct: string | null }
export interface HistoryPoint { date: string; value: string }
export interface Holding { symbol: string; units: string; price: string | null; price_date: string | null; book_cost: string | null }
export interface Position { symbol: string; units: string; book_cost?: string | null }

export interface Account {
  id: string;
  name: string;
  institution?: string | null;
  kind: AccountKind;
  currency: string;
  tags?: string[];
  positions?: Position[] | null;
}

export interface AccountView extends Account {
  group: Group;
  liability: boolean;
  value: string;
  native_value: string;
  as_of: string | null;
  days_stale: number | null;
  missing_fx: boolean;
  missing_price: boolean;
  history: HistoryPoint[];
  holdings: Holding[];
}

export interface Point {
  date: string;
  net_worth: string;
  assets: string;
  liabilities: string;
  by_group: Partial<Record<Group, string>>;
}

export interface LedgerView {
  version: number;
  as_of: string;
  base_currency: string;
  ledger_path: string;
  problem: Diagnostic | null;
  totals: { assets: string; liabilities: string; net_worth: string; by_group: Partial<Record<Group, string>> };
  series: Point[];
  deltas: { previous: Delta | null; ytd: Delta | null; one_year: Delta | null };
  accounts: AccountView[];
  fx: Record<string, string>;
  events: { date: string; note: string }[];
}

export type Mutation =
  | { op: "upsert_account"; account: Account }
  | { op: "remove_account"; id: string }
  | { op: "set_balance"; date: string; account: string; value: string }
  | { op: "set_price"; symbol: string; date: string; price: string }
  | { op: "set_fx"; currency: string; rate: string }
  | { op: "record_position_change"; change: { date: string; account: string; symbol: string; units: string; price?: string | null; note?: string | null } }
  | { op: "add_event"; event: { date: string; note: string } };

export class ApiError extends Error {
  constructor(message: string, public status: number, public line: number | null = null) {
    super(message);
  }
}

async function fail(r: Response): Promise<never> {
  const e = await r.json().catch(() => ({ message: r.statusText, line: null }));
  throw new ApiError(e.message ?? r.statusText, r.status, e.line ?? null);
}

export async function getView(): Promise<LedgerView> {
  const r = await fetch("/api/view");
  if (!r.ok) await fail(r);
  return r.json();
}

export async function apply(mutations: Mutation[]): Promise<LedgerView> {
  const r = await fetch("/api/apply", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(mutations),
  });
  if (!r.ok) await fail(r);
  return r.json();
}

/** Calls `onChange` with the new version whenever the ledger changes. Returns a stop function. */
export function subscribe(onChange: (version: number) => void): () => void {
  const es = new EventSource("/api/events");
  es.addEventListener("change", (e) => onChange(Number((e as MessageEvent).data)));
  return () => es.close();
}
