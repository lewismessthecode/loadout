import { describe, expect, it } from "vitest";
import { prepareUpdateProgress, runUpdateQueue, type UpdateProgress } from "./updateQueue";
import type { UpdateOutcome } from "./types";

const backup = (name: string): UpdateOutcome => ({ backupPath: `/fixture/backups/${name}`, updatedAt: "2026-10-21T00:00:00Z" });

describe("selected update queue", () => {
  it("waits for each result and continues after an individual failure", async () => {
    const calls: string[] = [];
    const progress = new Map<string, UpdateProgress>();
    let finishFirst: (outcome: UpdateOutcome) => void = () => {};
    const first = new Promise<UpdateOutcome>((resolve) => { finishFirst = resolve; });
    const queue = runUpdateQueue({
      requests: [{ skillId: "one", planId: "plan-one" }, { skillId: "two", planId: "plan-two" }, { skillId: "three", planId: "plan-three" }],
      apply: async ({ planId }) => {
        calls.push(planId);
        if (planId === "plan-one") return first;
        if (planId === "plan-two") throw new Error("Installation changed. Check again.");
        return backup("three");
      },
      onProgress: (id, value) => { progress.set(id, value); },
    });
    expect(calls).toEqual(["plan-one"]);
    expect(progress.get("one")).toEqual({ status: "updating" });
    finishFirst(backup("one"));
    await queue;
    expect(calls).toEqual(["plan-one", "plan-two", "plan-three"]);
    expect(progress.get("one")).toEqual({ status: "updated", outcome: backup("one") });
    expect(progress.get("two")).toEqual({ status: "failed", message: "Error: Installation changed. Check again." });
    expect(progress.get("three")).toEqual({ status: "updated", outcome: backup("three") });
  });

  it("retains earlier successful backups when retrying only a failed skill", async () => {
    let progress = new Map<string, UpdateProgress>();
    await runUpdateQueue({
      requests: [{ skillId: "one", planId: "one" }, { skillId: "two", planId: "old-two" }],
      apply: async ({ skillId }) => { if (skillId === "two") throw new Error("stale"); return backup(skillId); },
      onProgress: (id, value) => { progress.set(id, value); },
    });
    const retry = [{ skillId: "two", planId: "new-two" }];
    progress = prepareUpdateProgress(progress, retry);
    expect(progress.get("one")).toEqual({ status: "updated", outcome: backup("one") });
    expect(progress.has("two")).toBe(false);
    await runUpdateQueue({ requests: retry, apply: async () => backup("two"), onProgress: (id, value) => { progress.set(id, value); } });
    expect([...progress.values()]).toEqual([{ status: "updated", outcome: backup("one") }, { status: "updated", outcome: backup("two") }]);
  });

  it("does not apply anything when the selection is empty", async () => {
    const calls: string[] = [];
    await runUpdateQueue({ requests: [], apply: async ({ planId }) => { calls.push(planId); return backup("unused"); }, onProgress: () => {} });
    expect(calls).toEqual([]);
  });
});
