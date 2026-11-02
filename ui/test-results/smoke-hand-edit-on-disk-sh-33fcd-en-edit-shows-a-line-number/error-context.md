# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: smoke.spec.ts >> hand edit on disk shows up within a second; a broken edit shows a line number
- Location: e2e/smoke.spec.ts:41:1

# Error details

```
Error: expect(locator).toHaveText(expected) failed

Locator:  locator('.hero .money.xl')
Expected: "$9,999"
Received: "$5,102"
Timeout:  1500ms

Call log:
  - Expect "toHaveText" locator('.hero .money.xl') with timeout 1500ms
  - waiting for locator('.hero .money.xl')
    16 × locator resolved to <span class="money xl svelte-x26f9w">$5,102</span>
       - unexpected value "$5,102"

```

```yaml
- text: $5,102
```

# Test source

```ts
  1  | import { expect, test } from "@playwright/test";
  2  | import { readFileSync, writeFileSync } from "node:fs";
  3  | import { join } from "node:path";
  4  | 
  5  | const dataDir = process.env.WW_E2E_DATA!;
  6  | const ledger = () => join(dataDir, "ledger.yaml");
  7  | 
  8  | test.describe.configure({ mode: "serial" });
  9  | 
  10 | test("first run: add an account, check in, the chart gets a point", async ({ page }) => {
  11 |   await page.goto("/#/");
  12 |   await expect(page.getByRole("heading", { name: "Add your first account" })).toBeVisible();
  13 |   await page.getByRole("link", { name: "Go to Check-in" }).click();
  14 | 
  15 |   await page.getByRole("button", { name: "+ Add account" }).click();
  16 |   await page.locator("#add-name").fill("TD Chequing");
  17 |   await page.locator("#add-institution").fill("TD");
  18 |   await page.getByRole("button", { name: "Add account" }).click();
  19 | 
  20 |   const input = page.getByLabel("TD Chequing new value");
  21 |   await expect(input).toBeFocused();
  22 |   await input.fill("5,102.10");
  23 |   await expect(page.locator(".foot .sum")).toContainText("1 account updated");
  24 |   await input.press("Enter");
  25 | 
  26 |   await expect(page).toHaveURL(/#\/$/);
  27 |   await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");
  28 |   await expect(page.locator(".uplot")).toBeVisible();
  29 |   const yaml = readFileSync(ledger(), "utf8");
  30 |   expect(yaml).toContain("td-chequing: 5102.10");
  31 | 
  32 |   // Second check-in on an earlier date: two points, carry-forward intact.
  33 |   await page.goto("/#/check-in");
  34 |   await page.locator("#ci-date").fill("2026-01-31");
  35 |   await page.getByLabel("TD Chequing new value").fill("4000");
  36 |   await page.getByRole("button", { name: "Save check-in" }).click();
  37 |   await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");
  38 |   await expect(page.locator(".hero .sub")).toContainText("+$1,102");
  39 | });
  40 | 
  41 | test("hand edit on disk shows up within a second; a broken edit shows a line number", async ({ page }) => {
  42 |   await page.goto("/#/");
  43 |   await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");
  44 | 
  45 |   const good = readFileSync(ledger(), "utf8");
  46 |   writeFileSync(ledger(), good.replace("td-chequing: 5102.10", "td-chequing: 9999.00"));
> 47 |   await expect(page.locator(".hero .money.xl")).toHaveText("$9,999", { timeout: 1500 });
     |                                                 ^ Error: expect(locator).toHaveText(expected) failed
  48 | 
  49 |   writeFileSync(ledger(), good.replace("td-chequing: 9999.00", "td-chequing: 5102.10") + "accounts: [oops\n");
  50 |   const banner = page.getByRole("alert");
  51 |   await expect(banner).toBeVisible({ timeout: 1500 });
  52 |   await expect(banner).toContainText(/line \d+/);
  53 |   await expect(page.locator(".hero .money.xl")).toHaveText("$9,999"); // last good state kept
  54 | 
  55 |   // Saving is refused while the file is broken, and the broken file is untouched.
  56 |   await page.goto("/#/check-in");
  57 |   await expect(page.getByRole("button", { name: "Save check-in" })).toBeDisabled();
  58 |   expect(readFileSync(ledger(), "utf8")).toContain("[oops");
  59 | 
  60 |   writeFileSync(ledger(), good);
  61 |   await page.goto("/#/");
  62 |   await expect(banner).toBeHidden({ timeout: 1500 });
  63 |   await expect(page.locator(".hero .money.xl")).toHaveText("$5,102");
  64 | });
  65 | 
```