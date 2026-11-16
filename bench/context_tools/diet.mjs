// Run the pinned, unmodified Diet functions. No model or client installation.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const { packText } = await import(pathToFileURL(resolve(process.argv[2], "diet/dist/pack.js")));
const fixtures = JSON.parse(readFileSync(0, "utf8"));
const results = [];
for (const fixture of fixtures) {
  const rounds = [];
  for (let repeat = 0; repeat < 3; repeat++) {
    const started = performance.now();
    try {
      const channels = {};
      for (const channel of ["stdout", "stderr"]) {
        channels[channel] = packText(fixture[channel], {
          budgetTokens: 1000,
          exactOnly: true,
          protectLines: fixture.protect.filter((line) => fixture[channel].split("\n").includes(line)),
          protectMode: "fail",
        });
      }
      rounds.push({ channels, elapsed_ms: performance.now() - started });
    } catch {
      rounds.push({ error: "protected_content_or_budget_rejected", elapsed_ms: performance.now() - started });
    }
  }
  results.push({ case: fixture.case, rounds });
}
let protectedBudgetRejected = false;
try {
  packText("REQUIRED " + "x".repeat(100), {
    budgetTokens: 1, exactOnly: true, protectLines: ["REQUIRED " + "x".repeat(100)], protectMode: "fail",
  });
} catch {
  protectedBudgetRejected = true;
}
process.stdout.write(JSON.stringify({ results, protectedBudgetRejected }));
