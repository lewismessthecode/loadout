import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useMaintenance } from "@/lib/maintenance";
import { api } from "@/lib/api";
import { invalidateCatalog } from "@/lib/reload";
import { agentLabel } from "@/lib/format";
import { useI18n } from "@/lib/i18n";
import type { AgentInfo } from "@/lib/types";
import { Button, Dialog } from "./ui";

export interface InstallPreset {
  repo?: string;
  skill?: string;
  installedAgents?: string[];
}

export function InstallDialog({
  agents,
  preset,
  onClose,
}: {
  agents: AgentInfo[];
  preset: InstallPreset;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const maintenance = useMaintenance();
  const targets = agents.filter((a) => a.installable);
  const [showOther, setShowOther] = useState(false);
  const detected = targets.filter((a) => a.detected);
  const other = targets.filter((a) => !a.detected);
  const [repo, setRepo] = useState(preset.repo ?? "");
  const [skill, setSkill] = useState(preset.skill ?? "");
  const [chosen, setChosen] = useState<Set<string>>(
    () => new Set(targets.filter((a) => a.detected && !preset.installedAgents?.includes(a.id)).map((a) => a.id)),
  );
  const [log, setLog] = useState<string[]>([]);
  const [state, setState] = useState<"idle" | "running" | "ok" | "failed">("idle");
  const logRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [log]);

  const run = async () => {
    if (chosen.size === 0 || !repo.trim()) return;
    setLog([]);
    setState("running");
    try {
      await maintenance.run("install", async () => {
        const operationId = crypto.randomUUID();
        const stop = await api.onInstallLog((event) => {
          if (event.operationId === operationId) setLog((lines) => [...lines, event.line]);
        });
        try {
          const code = await api.install(repo, [...chosen], operationId, skill || undefined);
          setState(code === 0 ? "ok" : "failed");
          if (code !== 0) setLog((lines) => [...lines, t("install.exitCode", { code })]);
        } finally {
          stop();
        }
      });
    } catch (error) {
      setLog((lines) => [...lines, String(error)]);
      setState("failed");
    }
    invalidateCatalog(qc);
  };

  const toggle = (id: string) =>
    setChosen((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  return (
    <Dialog title={t("install.title")} onClose={state === "running" ? () => {} : onClose} width={540}>
      <div className="space-y-3">
        <label className="block">
          <div className="mb-1 text-[12px] text-muted">{t("install.repo")}</div>
          <input
            autoFocus
            value={repo}
            onChange={(e) => setRepo(e.target.value)}
            placeholder={t("install.repoPlaceholder")}
            disabled={state === "running"}
            className="h-8 w-full rounded-md border border-line bg-bg px-2.5 text-[13px] outline-none focus:border-faint"
          />
        </label>
        <label className="block">
          <div className="mb-1 text-[12px] text-muted">{t("install.skillName")}</div>
          <input
            value={skill}
            onChange={(e) => setSkill(e.target.value)}
            disabled={state === "running"}
            className="h-8 w-full rounded-md border border-line bg-bg px-2.5 text-[13px] outline-none focus:border-faint"
          />
        </label>
        <div>
          <div className="mb-1 text-[12px] text-muted">{t("install.installTo")}</div>
          {detected.length === 0 && <p className="mb-2 text-[12px] text-muted">{t("install.noDetected")}</p>}
          <div className="flex flex-wrap gap-x-4 gap-y-1">
            {detected.map((agent) => (
              <label key={agent.id} className="flex items-center gap-1.5 text-[13px]">
                <input type="checkbox" checked={chosen.has(agent.id)} onChange={() => toggle(agent.id)} disabled={state === "running"} />
                {agentLabel(agent.id, agents, t)}
                {preset.installedAgents?.includes(agent.id) && <span className="text-[11px] text-faint">{t("install.alreadyInstalled")}</span>}
              </label>
            ))}
          </div>
          {other.length > 0 && (
            <div className="mt-2">
              <button type="button" onClick={() => setShowOther((value) => !value)} aria-expanded={showOther} className="text-[12px] text-muted hover:text-fg">
                {showOther ? t("common.showLess") : t("install.otherAgents")}
              </button>
              {showOther && <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1">
                {other.map((agent) => <label key={agent.id} className="flex items-center gap-1.5 text-[13px]">
                  <input type="checkbox" checked={chosen.has(agent.id)} onChange={() => toggle(agent.id)} disabled={state === "running"} />
                  {agentLabel(agent.id, agents, t)}
                </label>)}
              </div>}
            </div>
          )}
          <div className="mt-1 text-[11.5px] text-faint">{t("install.globalNote")}</div>
          {chosen.size === 0 && <div className="mt-1 text-[12px] text-muted">{t("install.chooseAgent")}</div>}
        </div>

        {log.length > 0 && (
          <pre
            ref={logRef}
            className="max-h-44 overflow-auto rounded-md bg-hover p-2.5 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap"
          >
            {log.join("\n")}
          </pre>
        )}

        <div className="flex items-center justify-between">
          <span className="text-[12px] text-muted">
            {state === "ok" && t("install.ok")}
            {state === "failed" && <span className="text-danger">{t("install.failed")}</span>}
          </span>
          <div className="flex gap-2">
            <Button onClick={onClose} disabled={state === "running"}>
              {state === "ok" ? t("install.done") : t("common.cancel")}
            </Button>
            <Button variant="primary" onClick={run} disabled={!repo.trim() || chosen.size === 0 || maintenance.busy !== null || state === "running"}>
              {state === "running" ? t("install.installing") : t("common.install")}
            </Button>
          </div>
        </div>
      </div>
    </Dialog>
  );
}
