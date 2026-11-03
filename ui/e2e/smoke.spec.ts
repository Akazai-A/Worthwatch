import { expect, test } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const dataDir = process.env.WW_E2E_DATA!;
const ledger = () => join(dataDir, "ledger.yaml");
const ledgerText = () => readFileSync(ledger(), "utf8");

test.describe.configure({ mode: "serial" });

test("first run: add an account, check in, the chart gets a point", async ({ page }) => {
  await page.goto("/#/");
  await expect(page.getByRole("heading", { name: "Add your first account" })).toBeVisible();
  await page.getByRole("link", { name: "Go to Check-in" }).click();

  await page.getByRole("button", { name: "+ Add account" }).click();
  await page.locator("#add-name").fill("TD Chequing");
  await page.locator("#add-institution").fill("TD");
  await page.getByRole("button", { name: "Add account" }).click();

  const input = page.getByLabel("TD Chequing new value");
  await expect(input).toBeFocused();
  await input.fill("5,102.10");
  await expect(page.locator(".foot .sum")).toContainText("1 account updated");
  await input.press("Enter");

  await expect(page).toHaveURL(/#\/$/);
  await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");
  await expect(page.locator(".uplot")).toBeVisible();
  const yaml = readFileSync(ledger(), "utf8");
  expect(yaml).toMatch(/td-chequing: 5102\.10?\n/);

  // Second check-in on an earlier date: two points, carry-forward intact.
  await page.goto("/#/check-in");
  await page.locator("#ci-date").fill("2026-01-31");
  await page.getByLabel("TD Chequing new value").fill("4000");
  await page.getByRole("button", { name: "Save check-in" }).click();
  await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");
  await expect(page.locator(".hero .sub")).toContainText("+$1,102");
});

test("hand edit on disk shows up within a second; a broken edit shows a line number", async ({ page }) => {
  await page.goto("/#/");
  await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");

  const good = readFileSync(ledger(), "utf8");
  const edited = good.replace(/td-chequing: 5102\.10?\n/, "td-chequing: 9999.00\n");
  expect(edited).not.toBe(good);
  writeFileSync(ledger(), edited);
  await expect(page.locator(".hero .money.xl")).toHaveText("$9,999", { timeout: 1500 });

  writeFileSync(ledger(), edited + "accounts: [oops\n");
  const banner = page.getByRole("alert");
  await expect(banner).toBeVisible({ timeout: 1500 });
  await expect(banner).toContainText(/line \d+/);
  await expect(page.locator(".hero .money.xl")).toHaveText("$9,999"); // last good state kept

  // Saving is refused while the file is broken, and the broken file is untouched.
  await page.goto("/#/check-in");
  await expect(page.getByRole("button", { name: "Save check-in" })).toBeDisabled();
  expect(readFileSync(ledger(), "utf8")).toContain("[oops");

  writeFileSync(ledger(), good);
  await page.goto("/#/");
  await expect(banner).toBeHidden({ timeout: 1500 });
  await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");
});

test("paste a holdings export into a new investment account; portfolio values it from the pasted prices", async ({ page }) => {
  await page.goto("/#/check-in");
  await page.getByRole("button", { name: "+ Add account" }).click();
  await page.locator("#add-name").fill("Wealthsimple TFSA");
  await page.locator("#add-kind").selectOption("tfsa");
  await page.locator("#add-symbols").fill("XEQT.TO");
  await page.getByRole("button", { name: "Add account" }).click();

  await page.locator("#paste-wealthsimple-tfsa-open").click();
  const paste = "XEQT\tiShares Core Equity ETF Portfolio\t426.5 shares\t$34.12\t$14,552.18\nZAG\tBMO Aggregate Bond Index ETF\t200 shares\t$14.05\t$2,810.00\nCash\t—\t$1,204.33\n";
  await page.getByLabel("Pasted holdings for Wealthsimple TFSA").fill(paste);
  const apply = page.locator("#apply-paste");
  await expect(apply).toHaveText("Apply 2 changes");
  await expect(page.locator("#paste-wealthsimple-tfsa")).toContainText("2 of 3 lines parsed");
  await expect(page.locator("#paste-wealthsimple-tfsa")).toContainText("ZAG.TO");
  await apply.click();
  await expect(apply).toBeHidden();

  // Trades and prices landed in the file; portfolio shows both positions valued, offline.
  const yaml = readFileSync(ledger(), "utf8");
  expect(yaml).toMatch(/symbol: ZAG\.TO\n\s+units: 200\n/);
  expect(yaml).toMatch(/from pasted holdings/);
  await page.goto("/#/portfolio");
  await expect(page.locator(".portfolio .hero .money.xl")).toHaveText("$17,362");
  await expect(page.locator(".portfolio tbody tr.sec")).toHaveCount(2);
  await expect(page.locator(".portfolio .top")).toContainText("offline · manual prices only");
  await expect(page.locator("#refresh")).toHaveCount(0);
});

test("a holdings file dropped into data/holdings shows up in Look-through, Exposure and Overlap", async ({ page }) => {
  const { mkdirSync } = await import("node:fs");
  mkdirSync(join(dataDir, "holdings"), { recursive: true });
  const csv = [
    "iShares Core Equity ETF Portfolio",
    'Fund Holdings as of,"Oct 30, 2026"',
    "",
    "Ticker,Name,Sector,Asset Class,Market Value,Weight (%),Location,Exchange,Market Currency",
    '"AAPL","APPLE INC","Information Technology","Equity","1","60.00","United States","NASDAQ","USD"',
    '"SHOP","SHOPIFY INC","Information Technology","Equity","1","30.00","Canada","Toronto Stock Exchange","CAD"',
    '"RY","ROYAL BANK OF CANADA","Financials","Equity","1","10.00","Canada","Toronto Stock Exchange","CAD"',
    "",
  ].join("\n");
  writeFileSync(join(dataDir, "holdings", "XEQT.TO.csv"), csv);

  await page.goto("/#/portfolio/look-through");
  await expect(page.getByRole("heading", { name: "No holdings files yet" })).toBeVisible();
  await page.locator("#refresh-holdings").click(); // offline: re-reads the folder
  await expect(page.locator(".treemap .cell")).toHaveCount(4); // 3 companies + ZAG.TO as itself
  await expect(page.locator(".hero .sub")).toContainText("4 companies via 1 fund");
  await expect(page.locator(".hero .sub")).toContainText("ZAG.TO has no holdings file");
  await page.getByRole("button", { name: /^Apple Inc,/ }).click();
  const detail = page.locator(".detail");
  await expect(detail).toContainText("Apple Inc");
  await expect(detail).toContainText("XEQT.TO");
  await expect(detail).toContainText("60.00% of XEQT.TO");
  // 426.5 units × $34.12 × 60% = $8,731
  await expect(detail.locator(".money.l")).toHaveText("$8,731");
  expect(readFileSync(join(dataDir, "holdings", "XEQT.TO", "2026-10-30.json"), "utf8")).toContain("\"source\": \"file\"");

  await page.goto("/#/portfolio/exposure");
  const sector = page.locator(".read").filter({ hasText: "Sector" });
  await expect(sector).toContainText("Technology");
  await sector.getByRole("button", { name: /^All \d+/ }).click();
  await expect(sector).toContainText("Unclassified"); // ZAG.TO, no file and no securities entry
  await page.getByRole("radio", { name: "As held" }).click();
  await expect(sector).toContainText("Funds");

  await page.goto("/#/portfolio/overlap");
  await expect(page.locator(".empty")).toContainText("at least two funds");
});

test("Changes compares the last check-in with today: bridge, accounts, holdings, and the fund feed", async ({ page }) => {
  await page.goto("/#/changes");
  // Check-ins so far: Jan 31 (4,000) and today (5,102.10 in TD Chequing), plus the pasted TFSA.
  await expect(page.locator(".changes .hero .money.xl")).toContainText("+$");
  await expect(page.locator(".range .dates")).toContainText("Jan 31 →");
  await expect(page.locator("svg.bridge")).toBeVisible();
  // The bridge is the account list: tab to a step for from → to.
  const step = page.getByRole("listitem", { name: /^TD Chequing/ });
  await expect(step).toBeVisible();
  await step.focus();
  const tip = page.locator(".fit .tip");
  await expect(tip).toContainText("$4,000");
  await expect(tip).toContainText("+$1,102");
  // The pasted positions count as trades (the account did not exist on Jan 31).
  const xeqt = page.locator(".hold tr", { hasText: "XEQT.TO" });
  await expect(xeqt).toContainText("426.5");
  await expect(xeqt).toContainText("+$14,552");
  await expect(page.locator(".hero .split")).toContainText("contributions & trades");
  // One holdings file per fund so far: nothing to compare yet.
  await expect(page.locator(".changes .empty-line").last()).toContainText("Only one holdings file per fund");

  // A custom range that starts before any data is clamped to the first known date.
  await page.getByRole("radio", { name: "1Y" }).click();
  await expect(page.locator(".range .dates")).toContainText("Jan 31 →");
  await page.getByRole("radio", { name: "Custom" }).click();
  await page.getByRole("textbox", { name: "From" }).fill("2026-01-31");
  await page.getByRole("textbox", { name: "To" }).fill("2026-02-15");
  await page.getByRole("textbox", { name: "To" }).dispatchEvent("change");
  await expect(page.getByRole("heading", { name: "One check-in so far" })).toBeHidden();
  await expect(page.locator(".range .dates")).toContainText("Feb 15, 2026 · 15 days");
  await expect(page.locator(".changes .hero .money.xl")).toHaveText("$0");
});

test("Chat: a message becomes a diff card, nothing is written until Approve; an MCP proposal waits for its own Approve", async ({ page, request }) => {
  await page.goto("/#/chat");
  await expect(page.locator(".prov")).toContainText("Offline · built-in parser");
  const before = readFileSync(ledger(), "utf8");

  const input = page.getByLabel("Message");
  await input.fill("td chequing 6000, bought 10 xeqt");
  await input.press("Enter");
  const card = page.locator(".plan").first();
  await expect(card).toContainText("TD Chequing");
  await expect(card).toContainText("6,000.00");
  await expect(card).toContainText("+897.90");
  await expect(card).toContainText("Wealthsimple TFSA · XEQT.TO");
  await expect(card).toContainText("436.5");
  expect(readFileSync(ledger(), "utf8")).toBe(before); // proposing writes nothing

  // A rejection is shown, never applied.
  await input.fill("sold 5 VTI in the tfsa");
  await input.press("Enter");
  await expect(page.locator(".line.rej")).toContainText("Wealthsimple TFSA holds no VTI");
  await expect(card.locator(".row").first()).toHaveAttribute("title", "from “td chequing 6000”");

  // Edit the cash figure, then approve: the edited value lands in the file.
  await card.getByRole("button", { name: "Edit" }).click();
  await card.getByLabel("TD Chequing proposed").fill("6,100");
  await expect(card).toContainText("+997.90");
  await card.getByRole("button", { name: "Approve 2" }).click();
  await expect(page.locator(".line.ok")).toContainText("Saved · 2 changes");
  await expect.poll(() => readFileSync(ledger(), "utf8")).toMatch(/td-chequing: 6100(\.0+)?\n/);
  expect(readFileSync(ledger(), "utf8")).toMatch(/units: 436\.5\n/);

  // MCP-style proposal: queued, badge on the rail, approve from the card.
  const r = await request.post("/api/propose", { data: { origin: "mcp", text: "td chq 6200" } });
  const body = await r.json();
  expect(body.pending.id).toBeGreaterThan(0);
  await expect(page.locator(".rail .ask .badge")).toHaveText("1");
  const pending = page.locator(".plan", { hasText: "Claude Desktop" });
  await expect(pending).toContainText("6,200.00");
  expect(readFileSync(ledger(), "utf8")).toMatch(/td-chequing: 6100/);
  await pending.getByRole("button", { name: "Approve" }).click();
  await expect(pending).toBeHidden();
  await expect(page.locator(".rail .badge")).toHaveCount(0);
  await expect.poll(() => readFileSync(ledger(), "utf8")).toMatch(/td-chequing: 6200(\.0+)?\n/);
  const s = await (await request.get(`/api/pending/${body.pending.id}`)).json();
  expect(s.status).toBe("approved");
});

test("Plan: a recurring rule moves the projection; ⌘K reaches the wall view; a backup can be made", async ({ page }) => {
  await page.goto("/#/plan");
  const hero = page.locator(".hero .money.xl");
  await expect(hero).toBeVisible();
  const before = await hero.textContent();

  await page.getByRole("button", { name: "+ Add rule" }).click();
  await page.getByLabel("Account").selectOption({ label: "TD Chequing" });
  await page.getByLabel("Amount", { exact: true }).fill("1000");
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.locator(".rules .r")).toContainText("TD Chequing");
  await expect(hero).not.toHaveText(before!);
  expect(readFileSync(ledger(), "utf8")).toMatch(/recurring:\n- account: td-chequing\n {2}amount: 1000/);

  await page.keyboard.press("Control+k");
  await page.getByLabel("Search").fill("wall");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#\/wall$/);
  await expect(page.locator(".wall .big")).toBeVisible();

  await page.goto("/#/backups");
  await page.getByRole("button", { name: "Back up now" }).click();
  await expect(page.getByRole("link", { name: "Download" }).first()).toBeVisible();
});

test("Budget: a tap needs the token, shows at once as provisional, and the statement confirms it once", async ({ page, request }) => {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const today = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  const off = -now.getTimezoneOffset();
  const tz = `${off < 0 ? "-" : "+"}${pad(Math.floor(Math.abs(off) / 60))}:${pad(Math.abs(off) % 60)}`;
  const card = "TD Aeroplan Visa";
  const tap = { merchant: "Pai Northern Thai", amount: "48.00", card, at: `${today}T12:30:00${tz}` };

  const setup = await request.post("/api/apply", {
    data: [
      { op: "upsert_account", account: { id: "td-visa", name: "TD Visa", kind: "credit_card", currency: "CAD" } },
      { op: "set_feed", feed: { mode: "tailscale", icloud_dir: null, tailscale_url: "https://mac-mini.example.ts.net" } },
    ],
  });
  expect(setup.ok()).toBeTruthy();

  // Without the token: 401, nothing saved.
  const denied = await request.post("/api/ingest/transaction", { data: tap });
  expect(denied.status()).toBe(401);
  const wrong = await request.post("/api/ingest/transaction", { data: tap, headers: { Authorization: "Bearer nope" } });
  expect(wrong.status()).toBe(401);
  expect((await (await request.get("/api/budget")).json()).txns).toHaveLength(0);

  // With it: saved, and a retry of the same tap (the Shortcut's queue) changes nothing.
  const token = (await (await request.get("/api/settings")).json()).token as string;
  const auth = { Authorization: `Bearer ${token}` };
  const sent = Date.now();
  const ok = await request.post("/api/ingest/transaction", { data: tap, headers: auth });
  expect(ok.status()).toBe(200);
  expect(await ok.text()).toContain("worthwatch_saved");
  const again = await (await request.post("/api/ingest/transaction", { data: tap, headers: auth })).json();
  expect(again).toMatchObject({ added: 0, duplicates: 1 });

  // The unlinked card asks for its account; once linked, the tap is provisional.
  await page.goto("/#/budget");
  await expect(page.getByText(`${card} tapped 1 time`)).toBeVisible();
  await page.getByRole("button", { name: "Link" }).click();
  const rows = page.locator(".tx");
  await expect(rows).toHaveCount(1);
  await expect(rows.first().getByLabel("Apple Pay tap, waiting for the statement")).toBeVisible();
  expect(Date.now() - sent).toBeLessThan(5000);

  // The statement posts the meal with a tip; import matches it to the tap.
  const csv = `Date,Description,Amount\n${today},PAI NORTHERN THAI KITCH TORONTO,56.64\n`;
  await page.getByRole("button", { name: "Import statement…" }).click();
  await page.locator(".imp select").first().selectOption({ label: "TD Visa" });
  await page.getByLabel("Statement text").fill(csv);
  await page.getByRole("button", { name: "Import 1 new" }).click();
  await expect(rows).toHaveCount(1);
  await expect(rows.first().getByLabel("Tap matched to the statement")).toBeVisible();
  await expect(rows.first()).toContainText("$56.64");

  // Importing the same file again adds nothing.
  await page.getByRole("button", { name: "Import statement…" }).click();
  await page.locator(".imp select").first().selectOption({ label: "TD Visa" });
  await page.getByLabel("Statement text").fill(csv);
  await expect(page.getByText("1 already imported")).toBeVisible();
  await expect(page.getByRole("button", { name: "Import 0 new" })).toBeDisabled();
  expect((await (await request.get("/api/budget")).json()).txns).toHaveLength(1);
});

test("Play: the game asks for a budget, then a goal; the jar and today's token show, and a tap's reply carries the notification", async ({ page, request }) => {
  await page.goto("/#/budget/play");
  await expect(page.getByRole("heading", { name: "The game needs a budget" })).toBeVisible();

  const setup = await request.post("/api/apply", {
    data: [
      { op: "set_budget_limit", category: "dining", amount: "600" },
      { op: "set_budget_limit", category: "groceries", amount: "900" },
    ],
  });
  expect(setup.ok()).toBeTruthy();

  await expect(page.getByRole("heading", { name: /^Pick a goal for / })).toBeVisible();
  await page.locator(".choice").first().click();
  await expect(page.getByRole("heading", { name: /^Pick a goal for / })).toHaveCount(0);
  await expect(page.locator(".jarbox")).toContainText(/of \$\d/);
  await expect(page.locator(".tile.today")).toHaveCount(1);
  const game = await (await request.get("/api/game")).json();
  expect(Number(game.goal)).toBeGreaterThan(0);
  expect(ledgerText()).toMatch(/game:\n\s+goal: /);

  // The Shortcut shows whatever the reply's notify line says.
  const token = (await (await request.get("/api/settings")).json()).token as string;
  const now = new Date();
  const tap = { merchant: "Balzac's", amount: "4.85", card: "TD Aeroplan Visa", at: now.toISOString() };
  const reply = await (await request.post("/api/ingest/transaction", { data: tap, headers: { Authorization: `Bearer ${token}` } })).json();
  expect(reply.notify).toMatch(/^\$\d+ (left|over) today.*\nJar \$/);
  const day = await request.get("/api/ingest/day", { headers: { Authorization: `Bearer ${token}` } });
  expect(day.status()).toBe(200);
  expect((await day.json()).text).toContain("Jar");
  expect((await request.get("/api/ingest/day")).status()).toBe(401);

  // The Budget tab stays for setup and links into the game.
  await page.goto("/#/budget");
  await page.getByRole("link", { name: "Play" }).click();
  await expect(page).toHaveURL(/#\/budget\/play\/\d{4}-\d{2}$/);
});

test("⌘⇧H hides every amount as stars, and shows them again", async ({ page }) => {
  await page.goto("/#/");
  const amount = page.locator(".hero .money.xl");
  await expect(amount).toHaveText(/^\$[\d,]+$/);
  await page.keyboard.press("Control+Shift+H");
  await expect(amount).toHaveText("*****");
  await expect(page.locator(".money").filter({ hasText: /\d/ })).toHaveCount(0);
  await expect(page.locator(".rail").getByRole("button", { name: /Show amounts/ })).toBeVisible();
  // Unit counts are personal too; share prices are public and stay.
  await page.goto("/#/portfolio");
  await expect(page.locator("table td.num").filter({ hasText: /^\*{5}$/ }).first()).toBeVisible();
  await page.goto("/#/");
  await page.keyboard.press("Control+Shift+H");
  await expect(amount).toHaveText(/^\$[\d,]+$/);

  // The eye under the big figure does the same.
  const eye = page.getByRole("button", { name: "Hide amounts" }).first();
  await eye.click();
  await expect(amount).toHaveText("*****");
  await expect(page.locator(".hero").getByRole("button", { name: "Show amounts" })).toHaveAttribute("aria-pressed", "true");
  await page.locator(".hero").getByRole("button", { name: "Show amounts" }).click();
  await expect(amount).toHaveText(/^\$[\d,]+$/);
});

test("Check-in: Enter in ⌘K doesn't save, and a live update doesn't move the cursor", async ({ page }) => {
  await page.goto("/#/check-in");
  const visa = page.getByLabel("TD Visa new value");
  await visa.fill("123");
  const before = readFileSync(ledger(), "utf8");

  // Enter inside the palette runs the palette item, never the check-in save.
  await page.keyboard.press("Control+K");
  await page.getByRole("combobox", { name: "Search" }).fill("Backups");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#\/backups$/);
  expect(readFileSync(ledger(), "utf8")).toBe(before);

  // A change on disk reloads the view; the field being typed in keeps focus.
  await page.goto("/#/check-in");
  await visa.click();
  await visa.fill("456");
  writeFileSync(ledger(), before + "\n# touched by the e2e test\n");
  await expect(page.getByText("touched", { exact: false })).toHaveCount(0);
  await page.waitForTimeout(1200);
  await expect(visa).toBeFocused();
});
