import * as fs from "node:fs";
import * as path from "node:path";
import { ROUND_OUT, roundId } from "./cards";
import { REPO } from "./pages";
import { serve } from "./server";

// The global setup of a round (round.config.ts): checks UX_ROUND, clears
// the round's raw output of an earlier run and serves the site. It leaves
// target/ux/raw and metrics.json, the gating suite's, as they are.
export default async function roundSetup() {
  roundId();
  fs.rmSync(path.join(REPO, ROUND_OUT), { recursive: true, force: true });
  return serve();
}
