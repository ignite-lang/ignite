import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "tests",
  outputDir: "../target/ux/test-results",
  globalSetup: "./lib/server.ts",
  globalTeardown: "./lib/collect.ts",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  timeout: 120_000,
  reporter: [["list"], ["./lib/summary-reporter.ts"]],
  use: {
    browserName: "chromium",
    headless: true,
    locale: "en-US",
    timezoneId: "UTC",
    serviceWorkers: "block",
    trace: "off",
  },
});
