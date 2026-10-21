import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nProvider, translate } from "@/lib/i18n";
import { MaintenanceProvider, mergeInspections, mergeLocalInspections } from "@/lib/maintenance";
import type { AgentInfo, Skill, UpdateInspection, UpdateState } from "@/lib/types";
import { InstallDialog } from "./InstallDialog";
import { UpdatePanel, unsupportedMessages } from "./UpdatePanel";
import { ScanStatus } from "./ScanStatus";
import { UpdatesDialog } from "./UpdatesDialog";

const agents: AgentInfo[] = [
  { id: "codex", label: "Codex", detected: true, installable: true, usageSupported: true, roots: [{ path: "/fixture/.codex/skills", available: false, kind: "skills" }] },
  { id: "cursor", label: "Cursor", detected: false, installable: true, usageSupported: true, roots: [] },
  { id: "windsurf", label: "Windsurf", detected: false, installable: false, usageSupported: false, roots: [] },
];
const skill: Skill = {
  id: "skill-1", name: "Test skill", dirName: "test", description: "Fixture", canonicalDir: "/fixture/.agents/skills/test",
  installs: [{ agent: "codex", path: "/fixture/.codex/skills/test", kind: "symlink", plugin: false }],
  source: { repo: "owner/repo", author: "owner", url: "https://github.com/owner/repo", kind: "github" },
  installedAt: null, updatedAt: null, tags: [], autoTags: [], usage: { total: 0, lastUsed: null, byAgent: {}, daily: {} },
};

function render(element: React.ReactNode, state: UpdateState = { status: "unchecked" }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  client.setQueryData<UpdateInspection[]>(["updates"], [{ skillId: skill.id, state }]);
  return renderToStaticMarkup(<QueryClientProvider client={client}><I18nProvider><MaintenanceProvider>{element}</MaintenanceProvider></I18nProvider></QueryClientProvider>);
}

beforeEach(() => {
  vi.stubGlobal("localStorage", { getItem: () => "en" });
  vi.stubGlobal("navigator", { languages: ["en"] });
});

describe("maintenance choices", () => {
  it("defaults to detected installable agents and keeps other targets collapsed", () => {
    const html = render(<InstallDialog agents={agents} preset={{ repo: "owner/repo" }} onClose={() => {}} />);
    expect(html).toContain("Codex");
    expect(html).toContain("Other supported agents");
    expect(html).not.toContain("Cursor");
    expect(html).not.toContain("Windsurf");
    expect(html).toContain('type="checkbox" checked=""');
  });

  it("requires an explicit choice when no agent is detected", () => {
    const html = render(<InstallDialog agents={agents.map((agent) => ({ ...agent, detected: false }))} preset={{ repo: "owner/repo" }} onClose={() => {}} />);
    expect(html).toContain("Choose at least one agent.");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Install<\/button>/);
  });

  it("shows configured missing roots without claiming successful reads", () => {
    const html = render(<ScanStatus agents={agents} onClose={() => {}} />);
    expect(html).toContain("/fixture/.codex/skills");
    expect(html).toContain("Not found");
    expect(html).toContain("Activity logs not supported");
  });
});

describe("update results", () => {
  it("offers a reviewed backup update only for an available plan", () => {
    const html = render(<UpdatePanel skill={skill} agents={agents} />, { status: "available", planId: "opaque", checkedAt: "2026-10-21T00:00:00Z", oldRevision: "old", newRevision: "new", affectedAgents: ["codex"] });
    expect(html).toContain("Update available");
    expect(html).toContain("Update and keep backup");
    expect(html).not.toContain("opaque");
  });

  it("distinguishes recorded upstream currency from local verification", () => {
    const html = render(<UpdatePanel skill={skill} agents={agents} />, { status: "current", checkedAt: "2026-10-21T00:00:00Z", revision: "current" });
    expect(html).toContain("Recorded upstream revision is current.");
    expect(html).toContain("Local files have not been verified against upstream.");
    expect(html).not.toContain("Update and keep backup");
  });

  it("explains unsupported installations and removes update actions", () => {
    const html = render(<UpdatePanel skill={skill} agents={agents} />, { status: "unsupported", reason: "Managed by the Codex plugin" });
    expect(html).toContain("Managed by the Codex plugin");
    expect(html).not.toContain("Check for update");
    expect(html).not.toContain("Update and keep backup");
  });

  it("offers retry after a failed check", () => {
    const html = render(<UpdatePanel skill={skill} agents={agents} />, { status: "failed", message: "GitHub request timed out" });
    expect(html).toContain("GitHub request timed out");
    expect(html).toContain("Check again");
    expect(html).not.toContain("Recorded upstream revision is current.");
  });

  it("localizes every backend unsupported reason in both languages", () => {
    expect(Object.keys(unsupportedMessages).sort()).toEqual(["pluginManaged", "untracked", "legacyLock", "unsupportedSource", "externalDirectory", "independentCopy", "ambiguousLock"].sort());
    for (const [reason, key] of Object.entries(unsupportedMessages)) {
      const english = translate("en", key);
      const chinese = translate("zh", key);
      expect(english).not.toBe(reason);
      expect(chinese).not.toBe(english);
      expect(chinese).toMatch(/[\u4e00-\u9fff]/);
    }
  });

  it("keeps other reviewed plans when a completed update triggers a local rescan", () => {
    const available: UpdateState = { status: "available", planId: "still-reviewed", checkedAt: "now", oldRevision: "old", newRevision: "new", affectedAgents: ["codex"] };
    const afterUpdate: UpdateInspection[] = [{ skillId: "updated", state: { status: "unchecked" } }, { skillId: "remaining", state: available }];
    const rescanned: UpdateInspection[] = [{ skillId: "updated", state: { status: "unchecked" } }, { skillId: "remaining", state: { status: "unchecked" } }];
    expect(mergeLocalInspections(afterUpdate, rescanned)).toEqual(afterUpdate);
    expect(mergeLocalInspections(afterUpdate, [{ skillId: "remaining", state: { status: "unsupported", reason: "untracked" } }])).toEqual([{ skillId: "remaining", state: { status: "unsupported", reason: "untracked" } }]);
  });

  it("replaces one inspected result while preserving other checked skills", () => {
    const previous: UpdateInspection[] = [{ skillId: "one", state: { status: "unchecked" } }, { skillId: "two", state: { status: "current", checkedAt: "now", revision: "sha" } }];
    expect(mergeInspections(previous, [{ skillId: "one", state: { status: "failed", message: "offline" } }])).toEqual([
      { skillId: "one", state: { status: "failed", message: "offline" } }, previous[1],
    ]);
  });
});


describe("available update review", () => {
  it("selects only available skills and discloses the whole-folder backup policy", () => {
    const other = { ...skill, id: "unsupported", name: "Untracked fixture" };
    const html = render(<UpdatesDialog skills={[skill, other]} agents={agents} onClose={() => {}} />, { status: "available", planId: "private-plan", checkedAt: "now", oldRevision: "1111111111111111", newRevision: "2222222222222222", affectedAgents: ["codex"] });
    expect(html).toContain("Select all available updates");
    expect(html).toContain("Update selected (1)");
    expect(html).toContain("The whole existing skill folder will be replaced.");
    expect(html).toContain("complete previous folder as a backup");
    expect(html).toContain("Codex");
    expect(html).not.toContain("Untracked fixture");
    expect(html).not.toContain("private-plan");
  });

  it("cannot start an empty update queue", () => {
    const html = render(<UpdatesDialog skills={[skill]} agents={agents} onClose={() => {}} />);
    expect(html).toContain("No updates are available.");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Update selected \(0\)<\/button>/);
  });
});
