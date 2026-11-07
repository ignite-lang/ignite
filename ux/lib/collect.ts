import * as fs from "node:fs";
import * as path from "node:path";
import { OUT_DIR } from "../adapter";
import { REPO } from "./pages";

type Raw = { group: "gating" | "measured"; scenario: string; metrics: Record<string, number> };

// Gathers the raw records into target/ux/metrics.json: `scenarios` holds the
// gating values the ratchet reads, `measured` the measured-only viewports,
// reported and never gated.
export default async function globalTeardown() {
  const dir = path.join(REPO, OUT_DIR, "raw");
  const out: { scenarios: Record<string, Record<string, number>>; measured: Record<string, Record<string, number>> } = {
    scenarios: {},
    measured: {},
  };
  const files = fs.existsSync(dir) ? fs.readdirSync(dir).sort() : [];
  for (const file of files) {
    const raw = JSON.parse(fs.readFileSync(path.join(dir, file), "utf8")) as Raw;
    const target = raw.group === "gating" ? out.scenarios : out.measured;
    const metrics = (target[raw.scenario] ??= {});
    for (const [key, value] of Object.entries(raw.metrics)) {
      metrics[key] = (metrics[key] ?? 0) + value;
    }
  }
  const sorted = (record: Record<string, Record<string, number>>) =>
    Object.fromEntries(
      Object.keys(record)
        .sort()
        .map((scenario) => [
          scenario,
          Object.fromEntries(Object.keys(record[scenario]).sort().map((key) => [key, record[scenario][key]])),
        ]),
    );
  const metrics = { scenarios: sorted(out.scenarios), measured: sorted(out.measured) };
  fs.writeFileSync(path.join(REPO, OUT_DIR, "metrics.json"), JSON.stringify(metrics, null, 2) + "\n");
  const lines = ["measured-only viewports (reported, not gated):"];
  for (const [scenario, values] of Object.entries(metrics.measured)) {
    lines.push(`  ${scenario}: ${Object.entries(values).map(([k, v]) => `${k}=${v}`).join(" ")}`);
  }
  console.log(lines.join("\n"));
}
