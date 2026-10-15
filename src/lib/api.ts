import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentInfo, RemoveReport, Skill, SkillDetail } from "./types";

export const api = {
  agents: () => invoke<AgentInfo[]>("list_agents"),
  skills: () => invoke<Skill[]>("list_skills"),
  detail: (id: string) => invoke<SkillDetail>("get_skill", { id }),
  heatmap: (id: string | null, days: number) =>
    invoke<{ day: string; count: number }[]>("usage_heatmap", { id, days }),
  setTags: (id: string, tags: string[]) => invoke<void>("set_tags", { id, tags }),
  reveal: (path: string) => invoke<void>("reveal_in_finder", { path }),
  planRemoval: (id: string, paths?: string[]) => invoke<string[]>("plan_removal", { id, paths }),
  remove: (id: string, paths?: string[]) => invoke<RemoveReport>("remove_skill", { id, paths }),
  install: (repo: string, agents: string[], skill?: string) =>
    invoke<number>("install", { repo, agents, skill }),
  onInstallLog: (cb: (line: string) => void) => listen<string>("install-log", (e) => cb(e.payload)),
};
