import { useEffect, useState } from "react";
import { FolderOpen, RefreshCw } from "lucide-react";
import { api } from "@/lib/api";
import { agentLabel } from "@/lib/format";
import { useI18n, type MessageKey } from "@/lib/i18n";
import { useMaintenance } from "@/lib/maintenance";
import type { AgentInfo, Skill, UpdateState } from "@/lib/types";
import { Button, Dialog } from "./ui";

export const unsupportedMessages: Record<string, MessageKey> = {
  pluginManaged: "update.reason.pluginManaged",
  untracked: "update.reason.untracked",
  legacyLock: "update.reason.legacyLock",
  unsupportedSource: "update.reason.unsupportedSource",
  externalDirectory: "update.reason.externalDirectory",
  independentCopy: "update.reason.independentCopy",
  ambiguousLock: "update.reason.ambiguousLock",
};

export function UpdatePanel({ skill, agents }: { skill: Skill; agents: AgentInfo[] }) {
  const { t, rel } = useI18n();
  const maintenance = useMaintenance();
  const state: UpdateState = maintenance.states.get(skill.id) ?? { status: "unchecked" };
  const outcome = maintenance.outcomes.get(skill.id);
  const [review, setReview] = useState<Extract<UpdateState, { status: "available" }> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const checking = maintenance.checking.has(skill.id);
  const updating = maintenance.busy === "update" && review !== null;
  const reviewedPlanIsCurrent = review !== null && state.status === "available" && state.planId === review.planId;
  useEffect(() => {
    if (review && !reviewedPlanIsCurrent && !updating) setReview(null);
  }, [review, reviewedPlanIsCurrent, updating]);
  const apply = async () => {
    if (!review) return;
    setError(null);
    try {
      await maintenance.apply(skill.id, review.planId);
      setReview(null);
    } catch (failure) {
      setError(String(failure));
      setReview(null);
    }
  };

  return (
    <section className="border-t border-line px-5 py-4">
      <h3 className="mb-2 text-[11px] font-medium tracking-wide text-faint uppercase">{t("update.section")}</h3>
      <div className="space-y-2 text-[12px] text-muted" aria-live="polite">
        {outcome && <div className="rounded-md border border-line p-2.5">
          <p className="text-fg">{t("update.updated")}</p>
          <button onClick={() => api.reveal(outcome.backupPath)} title={outcome.backupPath} className="mt-1.5 inline-flex items-center gap-1 hover:text-fg">
            <FolderOpen size={12} /> {t("update.backup")}
          </button>
          <p className="selectable mt-1 break-all font-mono text-[10.5px]">{outcome.backupPath}</p>
        </div>}
        {state.status === "unchecked" && <p>{t("update.unchecked")}</p>}
        {state.status === "unsupported" && <p>{unsupportedMessages[state.reason] ? t(unsupportedMessages[state.reason]) : state.reason}</p>}
        {state.status === "current" && <div><p className="text-fg">{t("update.current")}</p><p className="mt-1">{t("update.checkedAt", { when: rel(state.checkedAt) })}</p></div>}
        {state.status === "available" && <p className="text-fg">{t("update.available")}</p>}
        {state.status === "failed" && <p className="selectable text-danger">{state.message}</p>}
        {state.status !== "unsupported" && <p className="text-[11.5px]">{t("update.unverified")}</p>}
        {maintenance.inspectionError && <p className="selectable text-danger">{maintenance.inspectionError}</p>}
        {error && <p className="selectable text-danger">{error}</p>}
        <div className="flex flex-wrap gap-2">
          {state.status !== "unsupported" && <Button disabled={checking || maintenance.busy !== null} onClick={() => { setError(null); void maintenance.check([skill.id]); }}>
            <RefreshCw size={12} className={checking ? "animate-spin" : ""} />
            {checking ? t("update.checking") : state.status === "failed" ? t("update.retry") : t("update.check")}
          </Button>}
          {state.status === "available" && <Button variant="primary" disabled={checking || maintenance.busy !== null} onClick={() => { setError(null); setReview(state); }}>{t("update.apply")}</Button>}
        </div>
      </div>
      {review && <Dialog title={t("update.review")} onClose={updating ? () => {} : () => setReview(null)}>
        <p className="text-[13px] leading-relaxed text-muted">{t("update.reviewBody")}</p>
        <p className="mt-3 text-[12px] font-medium">{skill.name}</p>
        <p className="mt-1 text-[11.5px] text-muted">{t("update.affects")}</p>
        <ul className="mt-1 list-inside list-disc text-[12px]">
          {review.affectedAgents.map((agent) => <li key={agent}>{agentLabel(agent, agents, t)}</li>)}
        </ul>
        <p className="mt-3 text-[11.5px] text-muted">{t("update.revision")}</p>
        <div className="selectable mt-1 break-all rounded-md bg-hover p-2 font-mono text-[10.5px]">{review.oldRevision}<br />→ {review.newRevision}</div>
        <div className="mt-4 flex justify-end gap-2">
          <Button disabled={updating} onClick={() => setReview(null)}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={maintenance.busy !== null || checking || !reviewedPlanIsCurrent} onClick={() => void apply()}>{updating ? t("update.updating") : t("update.apply")}</Button>
        </div>
      </Dialog>}
    </section>
  );
}
