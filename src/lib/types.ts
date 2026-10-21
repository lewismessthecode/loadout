export interface Install {
  agent: string;
  path: string;
  kind: "dir" | "symlink";
  plugin: boolean;
}

export interface Source {
  repo: string;
  author: string;
  url: string;
  kind: "github" | "wellKnown" | "plugin";
}

export interface Usage {
  total: number;
  lastUsed: string | null;
  byAgent: Record<string, number>;
  daily: Record<string, number>;
}

export interface Skill {
  id: string;
  name: string;
  dirName: string;
  description: string;
  canonicalDir: string;
  installs: Install[];
  source: Source | null;
  installedAt: string | null;
  updatedAt: string | null;
  tags: string[];
  autoTags: string[];
  usage: Usage;
}

export interface AgentInfo {
  id: string;
  label: string;
  detected: boolean;
  installable: boolean;
  usageSupported: boolean;
  roots: ScanRoot[];
}

export interface SkillDetail {
  skill: Skill;
  skillMd: string;
  files: string[];
}

export interface RemoveReport {
  removed: string[];
  lockEntriesRemoved: number;
}

export interface ScanRoot {
  path: string;
  available: boolean;
  kind: "skills" | "pluginCache";
}

export type UpdateState =
  | { status: "unchecked" }
  | { status: "unsupported"; reason: string }
  | { status: "current"; checkedAt: string; revision: string }
  | { status: "available"; planId: string; checkedAt: string; oldRevision: string; newRevision: string; affectedAgents: string[] }
  | { status: "failed"; message: string };

export interface UpdateInspection {
  skillId: string;
  state: UpdateState;
}

export interface UpdateOutcome {
  backupPath: string;
  updatedAt: string;
}

export interface InstallLog {
  operationId: string;
  line: string;
}
