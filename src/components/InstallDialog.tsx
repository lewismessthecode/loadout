import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
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
  const targets = agents.filter((a) => a.installable);
  const [repo, setRepo] = useState(preset.repo ?? "");
  const [skill, setSkill] = useState(preset.skill ?? "");
  const [chosen, setChosen] = useState<Set<string>>(
    () => new Set(targets.filter((a) => a.detected && !preset.installedAgents?.includes(a.id)).map((a) => a.id)),
  );
  const [log, setLog] = useState<string[]>([]);
  const [state, setState] = useState<"idle" | "running" | "ok" | "failed">("idle");
  const logRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    const un = api.onInstallLog((line) => setLog((l) => [...l, line]));
    return () => void un.then((f) => f());
  }, []);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [log]);

  const run = async () => {
    setLog([]);
    setState("running");
    try {
      const code = await api.install(repo, [...chosen], skill || undefined);
      setState(code === 0 ? "ok" : "failed");
      if (code !== 0) setLog((l) => [...l, t("install.exitCode", { code })]);
    } catch (e) {
      setLog((l) => [...l, String(e)]);
      setState("failed");
    }
    qc.invalidateQueries({ queryKey: ["skills"] });
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
          <div className="flex flex-wrap gap-x-4 gap-y-1">
            {targets.map((a) => (
              <label key={a.id} className="flex items-center gap-1.5 text-[13px]">
                <input type="checkbox" checked={chosen.has(a.id)} onChange={() => toggle(a.id)} disabled={state === "running"} />
                {agentLabel(a.id, agents, t)}
                {preset.installedAgents?.includes(a.id) && <span className="text-[11px] text-faint">{t("install.alreadyInstalled")}</span>}
              </label>
            ))}
          </div>
          <div className="mt-1 text-[11.5px] text-faint">{t("install.globalNote")}</div>
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
            <Button variant="primary" onClick={run} disabled={!repo.trim() || state === "running"}>
              {state === "running" ? t("install.installing") : t("common.install")}
            </Button>
          </div>
        </div>
      </div>
    </Dialog>
  );
}
