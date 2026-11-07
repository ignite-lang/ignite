import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "round",
  outputDir: "../target/ux/round/test-results",
  globalSetup: "./lib/round-setup.ts",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  timeout: 600_000,
  reporter: [["list"]],
  use: {
    browserName: "chromium",
    headless: true,
    locale: "en-US",
    timezoneId: "UTC",
    serviceWorkers: "block",
    trace: "off",
  },
});
