import type { MessageKey } from "@/lib/i18n";
import type { AgentInfo } from "@/lib/types";

export function homeRelative(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}

export function agentLabel(id: string, agents: AgentInfo[], t: (key: MessageKey) => string): string {
  if (id === "shared") return t("common.sharedAgent");
  return agents.find((a) => a.id === id)?.label ?? id;
}
