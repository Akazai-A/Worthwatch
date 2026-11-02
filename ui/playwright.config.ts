import { defineConfig } from "@playwright/test";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Smoke flow against the real binary: a fresh data dir per run, the built UI embedded.
// Build first: pnpm build && cargo build --bin worthwatch (CI does both).
const dataDir = process.env.WW_E2E_DATA ?? mkdtempSync(join(tmpdir(), "ww-e2e-"));
process.env.WW_E2E_DATA = dataDir; // workers read it from the environment
const port = 7781;

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    colorScheme: "dark",
    ...(process.env.PW_CHROMIUM ? { launchOptions: { executablePath: process.env.PW_CHROMIUM } } : {}),
  },
  webServer: {
    command: `../target/debug/worthwatch`,
    url: `http://127.0.0.1:${port}/api/health`,
    reuseExistingServer: false,
    env: { WORTHWATCH_DATA: dataDir, WORTHWATCH_BIND: `127.0.0.1:${port}`, WW_E2E_DATA: dataDir },
  },
  metadata: { dataDir },
});
