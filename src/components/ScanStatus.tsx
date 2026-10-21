import { useI18n } from "@/lib/i18n";
import { agentLabel, homeRelative } from "@/lib/format";
import type { AgentInfo } from "@/lib/types";
import { Dialog } from "./ui";

export function ScanStatus({ agents, onClose }: { agents: AgentInfo[]; onClose: () => void }) {
  const { t } = useI18n();
  return <Dialog title={t("scan.title")} onClose={onClose} width={580}>
    <p className="mb-4 text-[12px] leading-relaxed text-muted">{t("scan.note")}</p>
    <div className="space-y-4">
      {agents.map((agent) => <section key={agent.id}>
        <div className="flex items-center justify-between gap-2 text-[12px]">
          <h3 className="font-medium">{agentLabel(agent.id, agents, t)}</h3>
          <span className="text-[11px] text-muted">{t(agent.usageSupported ? "scan.usageSupported" : "scan.usageUnsupported")}</span>
        </div>
        <ul className="mt-1 space-y-1">
          {agent.roots.map((root) => <li key={root.path} className="flex items-start justify-between gap-3 text-[11.5px]">
            <span title={root.path} className="selectable break-all font-mono text-muted">{homeRelative(root.path)}</span>
            <span className="shrink-0 text-faint">{t(root.available ? "scan.available" : "scan.missing")}</span>
          </li>)}
        </ul>
      </section>)}
    </div>
  </Dialog>;
}
