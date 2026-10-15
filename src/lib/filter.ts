import type { Skill } from "./types";

export type View = "all" | "recent" | "stale" | "unknown" | "plugins";

export type Filter =
  | { kind: "view"; view: View }
  | { kind: "agent"; id: string }
  | { kind: "source"; repo: string }
  | { kind: "tag"; tag: string };

export type SortKey = "name" | "installed" | "lastUsed" | "total";
export interface Sort {
  key: SortKey;
  dir: "asc" | "desc";
}

export const DAY_MS = 86_400_000;
export const RECENT_DAYS = 30;

export const ALL: Filter = { kind: "view", view: "all" };

export function sameFilter(a: Filter, b: Filter): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function usedWithin(s: Skill, days: number, now: number): boolean {
  return s.usage.lastUsed !== null && now - Date.parse(s.usage.lastUsed) <= days * DAY_MS;
}

export const hasPlugin = (s: Skill) => s.installs.some((i) => i.plugin);

export function matchesFilter(s: Skill, f: Filter, now: number): boolean {
  switch (f.kind) {
    case "view":
      switch (f.view) {
        case "all":
          return true;
        case "recent":
          return usedWithin(s, RECENT_DAYS, now);
        case "stale":
          return !usedWithin(s, RECENT_DAYS, now);
        case "unknown":
          return s.source === null;
        case "plugins":
          return hasPlugin(s);
      }
    case "agent":
      return s.installs.some((i) => i.agent === f.id && !i.plugin);
    case "source":
      return s.source?.repo === f.repo;
    case "tag":
      return s.tags.includes(f.tag) || s.autoTags.includes(f.tag);
  }
}

export function matchesQuery(s: Skill, q: string): boolean {
  const needle = q.trim().toLowerCase();
  if (!needle) return true;
  const hay = [s.name, s.description, s.source?.repo ?? "", ...s.tags, ...s.autoTags].join("\n").toLowerCase();
  return needle.split(/\s+/).every((w) => hay.includes(w));
}

function sortValue(s: Skill, key: SortKey): string | number {
  switch (key) {
    case "name":
      return s.name.toLowerCase();
    case "installed":
      return s.installedAt ? Date.parse(s.installedAt) : 0;
    case "lastUsed":
      return s.usage.lastUsed ? Date.parse(s.usage.lastUsed) : 0;
    case "total":
      return s.usage.total;
  }
}

export function applyFilters(skills: Skill[], f: Filter, q: string, sort: Sort, now: number): Skill[] {
  const out = skills.filter((s) => matchesFilter(s, f, now) && matchesQuery(s, q));
  const sign = sort.dir === "asc" ? 1 : -1;
  return out.sort((a, b) => {
    const [x, y] = [sortValue(a, sort.key), sortValue(b, sort.key)];
    if (x < y) return -sign;
    if (x > y) return sign;
    return a.name.localeCompare(b.name);
  });
}

export interface Counts {
  views: Record<View, number>;
  agents: Record<string, number>;
  sources: [string, number][];
  tags: [string, number][];
}

export function countAll(skills: Skill[], now: number): Counts {
  const views: Record<View, number> = { all: 0, recent: 0, stale: 0, unknown: 0, plugins: 0 };
  const agents: Record<string, number> = {};
  const sources = new Map<string, number>();
  const tags = new Map<string, number>();
  for (const s of skills) {
    for (const v of Object.keys(views) as View[]) {
      if (matchesFilter(s, { kind: "view", view: v }, now)) views[v]++;
    }
    for (const a of new Set(s.installs.filter((i) => !i.plugin).map((i) => i.agent))) {
      agents[a] = (agents[a] ?? 0) + 1;
    }
    if (s.source && s.source.kind !== "plugin") {
      sources.set(s.source.repo, (sources.get(s.source.repo) ?? 0) + 1);
    }
    for (const t of new Set([...s.tags, ...s.autoTags])) tags.set(t, (tags.get(t) ?? 0) + 1);
  }
  const ranked = (m: Map<string, number>): [string, number][] =>
    [...m.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  return { views, agents, sources: ranked(sources), tags: ranked(tags) };
}
