import assert from "node:assert/strict";
import test from "node:test";
import {
  candidateSelectionKey,
  initialInstallationSelection,
  isConnectionPlanExpired,
} from "../../src/lib/connectionPlanView.ts";

const installation = (installation_id) => ({ installation_id });

test("a single candidate is selected, while multiple candidates require an explicit choice", () => {
  assert.equal(initialInstallationSelection([installation("one")]), "one");
  assert.equal(initialInstallationSelection([installation("one"), installation("two")]), "");
  assert.equal(initialInstallationSelection([]), "");
});

test("candidate key ignores array/object identity but changes with candidate membership", () => {
  const first = [installation("two"), installation("one")];
  const second = [installation("one"), installation("two")].map((item) => ({ ...item }));
  assert.equal(candidateSelectionKey(first), candidateSelectionKey(second));
  assert.notEqual(candidateSelectionKey(first), candidateSelectionKey([installation("one")]));
});

test("preview expires at its five-minute deadline, not before it", () => {
  const plan = { expires_at_unix_seconds: 1_800_000_300 };
  assert.equal(isConnectionPlanExpired(plan, 1_800_000_299_999), false);
  assert.equal(isConnectionPlanExpired(plan, 1_800_000_300_000), true);
});
