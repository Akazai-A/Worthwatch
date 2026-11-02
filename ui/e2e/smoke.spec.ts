import { expect, test } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const dataDir = process.env.WW_E2E_DATA!;
const ledger = () => join(dataDir, "ledger.yaml");

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
