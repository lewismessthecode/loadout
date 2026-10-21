import { useState } from "react";
import { FolderOpen, RefreshCw } from "lucide-react";
import { api } from "@/lib/api";
import { agentLabel } from "@/lib/format";
import { useI18n } from "@/lib/i18n";
import { useMaintenance } from "@/lib/maintenance";
import type { AgentInfo, Skill } from "@/lib/types";
import { prepareUpdateProgress, type UpdateProgress, type UpdateRequest } from "@/lib/updateQueue";
import { unsupportedMessages } from "./UpdatePanel";
import { Button, Dialog } from "./ui";

export function UpdatesDialog({ skills, agents, onClose }: { skills: Skill[]; agents: AgentInfo[]; onClose: () => void }) {
  const { t } = useI18n();
  const maintenance = useMaintenance();
  const [rows] = useState(() => skills.filter((skill) => maintenance.states.get(skill.id)?.status === "available"));
  const [chosen, setChosen] = useState(() => new Set(rows.map((skill) => skill.id)));
  const [progress, setProgress] = useState<Map<string, UpdateProgress>>(new Map());
  const [running, setRunning] = useState(false);
  const [activeIds, setActiveIds] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const available = rows.filter((skill) => maintenance.states.get(skill.id)?.status === "available" && progress.get(skill.id)?.status !== "updated");
  const selected = available.filter((skill) => chosen.has(skill.id));
  const disabled = maintenance.busy !== null || maintenance.checking.size > 0;
  const finished = activeIds.filter((id) => { const item = progress.get(id); return item && item.status !== "updating"; }).length;

  const run = async () => {
    const requests: UpdateRequest[] = selected.flatMap((skill) => {
      const state = maintenance.states.get(skill.id);
      return state?.status === "available" ? [{ skillId: skill.id, planId: state.planId }] : [];
    });
    if (requests.length === 0) return;
    setError(null);
    setRunning(true);
    setActiveIds(requests.map((request) => request.skillId));
    setProgress((current) => prepareUpdateProgress(current, requests));
    try {
      await maintenance.applySelected(requests, (skillId, item) => {
        setProgress((current) => new Map(current).set(skillId, item));
      });
    } catch (failure) {
      setError(String(failure));
    } finally {
      setRunning(false);
    }
  };

  const retry = async (skillId: string) => {
    await maintenance.check([skillId]);
    setProgress((current) => {
      const next = new Map(current);
      next.delete(skillId);
      return next;
    });
    setChosen((current) => new Set(current).add(skillId));
  };

  return <Dialog title={t("update.queueTitle")} onClose={running ? () => {} : onClose} width={600}>
    <p className="text-[12px] leading-relaxed text-muted">{t("update.reviewBody")}</p>
    <p className="mt-2 text-[11.5px] text-muted">{t("update.queueNote")}</p>
    {rows.length === 0 ? <p className="mt-4 text-[13px] text-muted">{t("update.queueEmpty")}</p> : <>
      <label className="mt-4 flex items-center gap-2 text-[12px]">
        <input type="checkbox" disabled={disabled || available.length === 0} checked={available.length > 0 && selected.length === available.length} onChange={() => setChosen(selected.length === available.length ? new Set() : new Set(available.map((skill) => skill.id)))} />
        {t("update.selectAll")}
      </label>
      <ul className="mt-2 divide-y divide-line rounded-md border border-line px-3" aria-label={t("update.queueTitle")}>
        {rows.map((skill) => {
          const state = maintenance.states.get(skill.id);
          const item = progress.get(skill.id);
          const outcome = item?.status === "updated" ? item.outcome : null;
          const failed = item?.status === "failed" ? item.message : state?.status === "failed" ? state.message : null;
          const canSelect = state?.status === "available" && item?.status !== "updated";
          return <li key={skill.id} className="py-3">
            <div className="flex items-start gap-2.5">
              <input type="checkbox" className="mt-0.5" aria-label={t("update.selectSkill", { name: skill.name })} disabled={disabled || !canSelect} checked={canSelect && chosen.has(skill.id)} onChange={() => setChosen((current) => {
                const next = new Set(current);
                next.has(skill.id) ? next.delete(skill.id) : next.add(skill.id);
                return next;
              })} />
              <div className="min-w-0 flex-1 text-[12px]">
                <p className="font-medium">{skill.name}</p>
                {state?.status === "available" && !outcome && <>
                  <p className="mt-1 text-[11.5px] text-muted">{state.affectedAgents.map((agent) => agentLabel(agent, agents, t)).join(", ")}</p>
                  <p title={`${state.oldRevision} → ${state.newRevision}`} className="selectable mt-1 break-all font-mono text-[10.5px] text-faint">{state.oldRevision.slice(0, 12)} → {state.newRevision.slice(0, 12)}</p>
                </>}
                {item?.status === "updating" && <p className="mt-1 text-muted" role="status">{t("update.updating")}</p>}
                {outcome && <div className="mt-1">
                  <p>{t("update.updated")}</p>
                  <button onClick={() => api.reveal(outcome.backupPath)} className="mt-1 inline-flex items-center gap-1 text-muted hover:text-fg" title={outcome.backupPath}><FolderOpen size={12} />{t("update.backup")}</button>
                  <p className="selectable mt-1 break-all font-mono text-[10.5px] text-faint">{outcome.backupPath}</p>
                </div>}
                {failed && <p className="selectable mt-1 break-words text-danger">{failed}</p>}
                {state?.status === "current" && !outcome && <p className="mt-1 text-muted">{t("update.current")}</p>}
                {state?.status === "unsupported" && <p className="mt-1 text-muted">{unsupportedMessages[state.reason] ? t(unsupportedMessages[state.reason]) : state.reason}</p>}
              </div>
              {failed && <Button disabled={disabled} onClick={() => void retry(skill.id)}><RefreshCw size={12} />{t("update.retry")}</Button>}
            </div>
          </li>;
        })}
      </ul>
    </>}
    <div className="mt-3 text-[12px] text-muted" aria-live="polite">{activeIds.length > 0 && t("update.queueProgress", { n: finished, total: activeIds.length })}</div>
    {error && <p className="selectable mt-2 text-[12px] text-danger">{error}</p>}
    <div className="mt-4 flex justify-end gap-2">
      <Button disabled={running} onClick={onClose}>{t("common.close")}</Button>
      <Button variant="primary" disabled={disabled || selected.length === 0} onClick={() => void run()}>{running ? t("update.updating") : t("update.updateSelected", { n: selected.length })}</Button>
    </div>
  </Dialog>;
}
