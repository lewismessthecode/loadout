import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { QueryClient } from "@tanstack/react-query";

export const SKILLS_CHANGED = "skills-changed";

export function onSkillsChanged(cb: () => void): Promise<UnlistenFn> {
  return listen(SKILLS_CHANGED, () => {
    cb();
  });
}

export function invalidateCatalog(client: QueryClient): void {
  void client.invalidateQueries({ queryKey: ["skills"] });
  void client.invalidateQueries({ queryKey: ["agents"] });
  void client.invalidateQueries({ queryKey: ["skill"] });
}
