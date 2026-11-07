import * as fs from "node:fs";
import * as path from "node:path";
import type { FullResult, Reporter, TestCase, TestResult } from "@playwright/test/reporter";
import { OUT_DIR } from "../adapter";
import { REPO } from "./pages";

// Writes target/ux/gating.json at the end of the gating suite: its status
// and counts, integers only, and the titles of the tests that failed. A
// round (site::round::write) trusts target/ux/metrics.json only next to
// this file, so a metrics.json of an earlier run is never reported.
export default class SummaryReporter implements Reporter {
  private passed = 0;
  private failed: string[] = [];
  private flaky = 0;
  private skipped = 0;
  private file = path.join(REPO, OUT_DIR, "gating.json");

  onBegin() {
    fs.rmSync(this.file, { force: true });
  }

  onTestEnd(test: TestCase, result: TestResult) {
    const outcome = test.outcome();
    if (result.retry < test.retries && outcome === "unexpected") {
      return;
    }
    if (outcome === "expected") {
      this.passed++;
    } else if (outcome === "flaky") {
      this.flaky++;
    } else if (outcome === "skipped") {
      this.skipped++;
    } else {
      this.failed.push(test.titlePath().filter(Boolean).join(" › "));
    }
  }

  onEnd(result: FullResult) {
    fs.mkdirSync(path.dirname(this.file), { recursive: true });
    const summary = {
      status: result.status,
      passed: this.passed,
      failed: this.failed.length,
      flaky: this.flaky,
      skipped: this.skipped,
      failed_tests: [...this.failed].sort(),
    };
    fs.writeFileSync(this.file, JSON.stringify(summary, null, 2) + "\n");
  }
}
