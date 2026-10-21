import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentInfo, InstallLog, RemoveReport, Skill, SkillDetail, UpdateInspection, UpdateOutcome } from "./types";

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
  inspectUpdates: () => invoke<UpdateInspection[]>("inspect_updates"),
  checkUpdates: (ids: string[]) => invoke<UpdateInspection[]>("check_updates", { ids }),
  applyUpdate: (planId: string) => invoke<UpdateOutcome>("apply_update", { planId }),
  install: (repo: string, agents: string[], operationId: string, skill?: string) =>
    invoke<number>("install", { repo, agents, skill, operationId }),
  onInstallLog: (cb: (event: InstallLog) => void) => listen<InstallLog>("install-log", (e) => cb(e.payload)),
};
