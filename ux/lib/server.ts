import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as net from "node:net";
import * as path from "node:path";
import { OUT_DIR, SITE_DIR } from "../adapter";
import { REPO } from "./pages";

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close(() => resolve(port));
    });
  });
}

async function ready(url: string, deadline: number) {
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch {
      // not listening yet
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`${url} did not answer within 10 s`);
}

// Serves the built site with miniserve on a free loopback port, clears the
// raw records of an earlier run, and hands the base URL to the workers.
export default async function globalSetup() {
  fs.rmSync(path.join(REPO, OUT_DIR, "raw"), { recursive: true, force: true });
  return serve();
}

// Serves the built site and sets UX_BASE_URL; returns the teardown.
export async function serve() {
  const port = await freePort();
  const child = spawn(
    "miniserve",
    ["--index", "index.html", "-i", "127.0.0.1", "-p", String(port), path.join(REPO, SITE_DIR)],
    { stdio: "ignore" },
  );
  const base = `http://127.0.0.1:${port}/`;
  await ready(base, Date.now() + 10_000);
  process.env.UX_BASE_URL = base;
  return async () => {
    child.kill();
  };
}
