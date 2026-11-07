import * as fs from "node:fs";
import * as path from "node:path";
import { OUT_DIR } from "../adapter";
import type { Mode } from "./bot";
import { type Step, tasks, UX_DIR } from "./pages";

// A scenario card of a round's trial (SKILL.md section 3.1): a persona who
// does several tasks in one session, with twists (a wrong turn, a theme
// switch) between them. An anchor card is the same in every round; a
// rotating card belongs to one round. The driver of ux/round/ plays it
// with the screen-only user of lib/bot.ts.
export type Twist = { id: string; before: string; why: string; route: Step[] };

export type Card = {
  id: string;
  kind: "anchor" | "rotating";
  theme: "light" | "dark";
  round?: string;
  persona: string;
  facts: string;
  mode: Mode;
  tasks: string[];
  between: Step[];
  twists: Twist[];
};

export const ROUND_OUT = path.join(OUT_DIR, "round");

// The round named by UX_ROUND, `R-` and two or more digits.
export function roundId(): string {
  const round = process.env.UX_ROUND ?? "";
  if (!/^R-\d{2,}$/.test(round)) {
    throw new Error(`UX_ROUND is ${JSON.stringify(round)}, not a round id like R-01: run task ux-round ROUND=R-01`);
  }
  return round;
}

// The cards of `round`: every anchor and the one rotating card of that
// round, in byte order of their ids. Refuses a card that names an unknown
// task, a twist before a task the card does not do, or a round with other
// than two anchors and one rotating card.
export function roundCards(round: string): Card[] {
  const dir = path.join(UX_DIR, "scenarios/cards");
  const known = new Set(tasks().map((task) => task.id));
  const all = fs
    .readdirSync(dir)
    .filter((name) => name.endsWith(".json"))
    .sort()
    .map((name) => {
      const card = JSON.parse(fs.readFileSync(path.join(dir, name), "utf8")) as Card;
      if (`${card.id}.json` !== name) {
        throw new Error(`ux/scenarios/cards/${name} names the id ${card.id}`);
      }
      for (const task of card.tasks) {
        if (!known.has(task)) {
          throw new Error(`card ${card.id} names the task ${task}, which ux/scenarios does not hold`);
        }
      }
      for (const twist of card.twists) {
        if (!card.tasks.includes(twist.before)) {
          throw new Error(`card ${card.id}: twist ${twist.id} comes before ${twist.before}, a task the card does not do`);
        }
      }
      return card;
    });
  const anchors = all.filter((card) => card.kind === "anchor");
  const rotating = all.filter((card) => card.kind === "rotating" && card.round === round);
  if (anchors.length !== 2 || rotating.length !== 1) {
    throw new Error(
      `round ${round} needs two anchor cards and one rotating card with "round": "${round}" under ux/scenarios/cards; found ${anchors.length} and ${rotating.length}`,
    );
  }
  return [...anchors, ...rotating];
}
