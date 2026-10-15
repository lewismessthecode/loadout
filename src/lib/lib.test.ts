import { describe, expect, it } from "vitest";
import { applyFilters, countAll, ALL } from "./filter";
import { buildGrid, level, monthLabels } from "./heat";
import type { Skill } from "./types";

const NOW = Date.parse("2026-10-15T12:00:00Z");

function skill(over: Partial<Skill> & { name: string }): Skill {
  return {
    id: over.name,
    dirName: over.name,
    description: "",
    canonicalDir: `/h/${over.name}`,
    installs: [{ agent: "shared", path: `/h/${over.name}`, kind: "dir", plugin: false }],
    source: null,
    installedAt: null,
    updatedAt: null,
    tags: [],
    autoTags: [],
    usage: { total: 0, lastUsed: null, byAgent: {}, daily: {} },
    ...over,
  };
}

const skills = [
  skill({ name: "tdd", source: { repo: "mattpocock/skills", author: "mattpocock", url: "", kind: "github" }, tags: ["testing"],
    usage: { total: 3, lastUsed: "2026-10-10T00:00:00Z", byAgent: {}, daily: {} } }),
  skill({ name: "old", usage: { total: 1, lastUsed: "2026-06-01T00:00:00Z", byAgent: {}, daily: {} } }),
  skill({ name: "plug", installs: [{ agent: "cursor", path: "/p", kind: "dir", plugin: true }],
    source: { repo: "cursor-public/pstack", author: "cursor-public", url: "", kind: "plugin" } }),
];

describe("filters", () => {
  it("splits recent from stale using last-use time", () => {
    const names = (view: "recent" | "stale") =>
      applyFilters(skills, { kind: "view", view }, "", { key: "name", dir: "asc" }, NOW).map((s) => s.name);
    expect(names("recent")).toEqual(["tdd"]);
    expect(names("stale")).toEqual(["old", "plug"]);
  });

  it("agent filter ignores plugin installs, plugins view finds them", () => {
    const sort = { key: "name", dir: "asc" } as const;
    expect(applyFilters(skills, { kind: "agent", id: "cursor" }, "", sort, NOW)).toHaveLength(0);
    expect(applyFilters(skills, { kind: "view", view: "plugins" }, "", sort, NOW).map((s) => s.name)).toEqual(["plug"]);
  });

  it("searches name, source and tags across all words", () => {
    const sort = { key: "name", dir: "asc" } as const;
    expect(applyFilters(skills, ALL, "mattpocock testing", sort, NOW).map((s) => s.name)).toEqual(["tdd"]);
    expect(applyFilters(skills, ALL, "nomatch", sort, NOW)).toHaveLength(0);
  });

  it("sorts by uses descending", () => {
    const r = applyFilters(skills, ALL, "", { key: "total", dir: "desc" }, NOW);
    expect(r.map((s) => s.name)).toEqual(["tdd", "old", "plug"]);
  });

  it("counts exclude plugin sources and agent plugin installs", () => {
    const c = countAll(skills, NOW);
    expect(c.views).toMatchObject({ all: 3, recent: 1, stale: 2, unknown: 1, plugins: 1 });
    expect(c.agents).toEqual({ shared: 2 });
    expect(c.sources).toEqual([["mattpocock/skills", 1]]);
    expect(c.tags).toEqual([["testing", 1]]);
  });
});

describe("heat grid", () => {
  const today = new Date(2026, 9, 15); // Thu 15 Oct 2026

  it("ends with the week containing today and flags future days", () => {
    const g = buildGrid({ "2026-10-15": 5, "2026-10-12": 1 }, 53, today);
    expect(g).toHaveLength(53);
    const last = g[52];
    expect(last[0].date).toBe("2026-10-11");
    expect(last[4]).toMatchObject({ date: "2026-10-15", count: 5, level: 3, future: false });
    expect(last[5].future).toBe(true);
    expect(last[1]).toMatchObject({ date: "2026-10-12", level: 1 });
  });

  it("maps counts to levels", () => {
    expect([0, 1, 2, 3, 4, 6, 7, 20].map(level)).toEqual([0, 1, 2, 2, 3, 3, 4, 4]);
  });

  it("labels months without crowding", () => {
    const labels = monthLabels(buildGrid({}, 53, today));
    expect(labels.length).toBeGreaterThanOrEqual(11);
    labels.forEach((l, i) => i > 0 && expect(l.col - labels[i - 1].col).toBeGreaterThanOrEqual(3));
  });
});
