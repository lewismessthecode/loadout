import { useEffect, useMemo, useRef, useState } from "react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { Plus, RefreshCw, Search, FolderSearch } from "lucide-react";
import { api } from "@/lib/api";
import { invalidateCatalog, onSkillsChanged } from "@/lib/reload";
import { ALL, applyFilters, countAll, type Filter, type Sort, type SortKey, type View } from "@/lib/filter";
import { agentLabel } from "@/lib/format";
import { useI18n, type MessageKey } from "@/lib/i18n";
import type { AgentInfo, Skill } from "@/lib/types";
import { Sidebar } from "@/components/Sidebar";
import { SkillList } from "@/components/SkillList";
import { DetailPanel } from "@/components/DetailPanel";
import { InstallDialog, type InstallPreset } from "@/components/InstallDialog";
import { UpdatesDialog } from "@/components/UpdatesDialog";
import { ScanStatus } from "@/components/ScanStatus";
import { MaintenanceProvider, useMaintenance } from "@/lib/maintenance";
import { Button } from "@/components/ui";

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: true, retry: false } },
});

const VIEW_TITLES: Record<View, MessageKey> = {
  all: "sidebar.views.all",
  recent: "sidebar.views.recent",
  stale: "app.filter.stale",
  unknown: "sidebar.views.unknown",
  plugins: "sidebar.views.plugins",
};

function filterTitle(f: Filter, agents: AgentInfo[], t: (key: MessageKey) => string): string {
  switch (f.kind) {
    case "view":
      return t(VIEW_TITLES[f.view]);
    case "agent":
      return agentLabel(f.id, agents, t);
    case "source":
      return f.repo;
    case "tag":
      return `#${f.tag}`;
  }
}

function Shell() {
  const { t } = useI18n();
  const skillsQ = useQuery({ queryKey: ["skills"], queryFn: api.skills });
  const agentsQ = useQuery({ queryKey: ["agents"], queryFn: api.agents, staleTime: Infinity });
  const maintenance = useMaintenance();

  const [filter, setFilter] = useState<Filter>(ALL);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>({ key: "name", dir: "asc" });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [scanOpen, setScanOpen] = useState(false);
  const [updatesOpen, setUpdatesOpen] = useState(false);
  const [install, setInstall] = useState<InstallPreset | null>(null);
  const search = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const pending = onSkillsChanged(() => invalidateCatalog(queryClient));
    return () => {
      void pending.then((stop) => stop());
    };
  }, []);

  const skills = skillsQ.data ?? [];
  const agents = agentsQ.data ?? [];
  const counts = useMemo(() => countAll(skills, Date.now()), [skills]);
  const visible = useMemo(() => applyFilters(skills, filter, query, sort, Date.now()), [skills, filter, query, sort]);
  const selected: Skill | null = visible.find((s) => s.id === selectedId) ?? null;
  const eligibleIds = visible.filter((skill) => maintenance.states.get(skill.id)?.status !== "unsupported").map((skill) => skill.id);
  const availableCount = skills.filter((skill) => maintenance.states.get(skill.id)?.status === "available").length;

  useEffect(() => {
    if (skillsQ.isSuccess && selectedId && !visible.some((skill) => skill.id === selectedId)) setSelectedId(null);
  }, [visible, selectedId, skillsQ.isSuccess]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (document.querySelector('[role="dialog"]')) return;
      const typing = e.target instanceof HTMLElement && (e.target.matches("input, textarea, select") || e.target.isContentEditable);
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        search.current?.focus();
      } else if (e.key === "Escape" && !typing) {
        setSelectedId(null);
      } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && !typing && visible.length > 0) {
        e.preventDefault();
        const i = visible.findIndex((s) => s.id === selectedId);
        const next = e.key === "ArrowDown" ? Math.min(i + 1, visible.length - 1) : Math.max(i - 1, 0);
        setSelectedId(visible[i === -1 ? 0 : next].id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible, selectedId]);

  const toggleSort = (key: SortKey) =>
    setSort((s) =>
      s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: key === "name" ? "asc" : "desc" },
    );

  const used = skills.filter((s) => s.usage.total > 0).length;

  return (
    <div className="flex h-screen">
      <Sidebar filter={filter} onFilter={setFilter} counts={counts} agents={agents} />

      <main className="flex min-w-0 flex-1 flex-col bg-bg">
        <div data-tauri-drag-region className="flex h-[52px] shrink-0 items-end gap-3 px-5 pb-2">
          <div className="pointer-events-none min-w-0 flex-1">
            <h1 className="truncate text-[15px] leading-none font-semibold">
              {filterTitle(filter, agents, t)}
            </h1>
          </div>
          <div className="relative min-w-[80px] w-[190px] shrink">
            <Search size={13} className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-faint" />
            <input
              ref={search}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("common.search")}
              className="h-7 w-full rounded-md border border-line bg-bg pr-2 pl-7 text-[13px] outline-none placeholder:text-faint focus:border-faint"
            />
          </div>
          <Button
            variant="ghost"
            aria-label={t("app.rescan")}
            title={t("app.rescanTitle")}
            onClick={() => {
              invalidateCatalog(queryClient);
            }}
            className="w-7 justify-center px-0"
          >
            <RefreshCw size={13} className={skillsQ.isFetching ? "animate-spin" : ""} />
          </Button>
          <Button variant="primary" disabled={maintenance.busy !== null} onClick={() => setInstall({})}>
            <Plus size={13} /> {t("common.install")}
          </Button>
        </div>

        <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-2 px-5 pt-1 pb-3 text-[12px] text-muted">
          <span>{t("app.summary", { n: visible.length, total: skills.length, used })}</span>
          {availableCount > 0 && <Button disabled={maintenance.busy !== null} onClick={() => setUpdatesOpen(true)}>{t("update.reviewAvailable", { n: availableCount })}</Button>}
          <Button disabled={maintenance.busy !== null || maintenance.checking.size > 0 || eligibleIds.length === 0} title={t("update.checkVisibleTitle")} onClick={() => void maintenance.check(eligibleIds)}>
            <RefreshCw size={13} className={maintenance.checking.size > 0 ? "animate-spin" : ""} /> {maintenance.checking.size > 0 ? t("update.checking") : t("update.checkVisible")}
          </Button>
          <span className="flex-1" />
          <button onClick={() => setScanOpen(true)} className="inline-flex items-center gap-1.5 text-muted hover:text-fg">
            <FolderSearch size={12} /> {t("scan.title")}
          </button>
        </div>

        {skillsQ.isLoading ? (
          <div className="grid flex-1 place-items-center text-muted">{t("app.scanning")}</div>
        ) : skillsQ.isError ? (
          <div className="selectable grid flex-1 place-items-center px-8 text-center text-danger">{String(skillsQ.error)}</div>
        ) : (
          <SkillList
            skills={visible}
            agents={agents}
            selectedId={selected?.id ?? null}
            onSelect={setSelectedId}
            sort={sort}
            onSort={toggleSort}
          />
        )}
      </main>

      {selected && (
        <DetailPanel
          key={selected.id}
          skill={selected}
          agents={agents}
          onClose={() => setSelectedId(null)}
          onInstallElsewhere={(s) =>
            setInstall({
              repo: s.source?.repo,
              skill: s.dirName,
              installedAgents: [...new Set(s.installs.map((i) => i.agent))],
            })
          }
        />
      )}

      {updatesOpen && <UpdatesDialog skills={skills} agents={agents} onClose={() => setUpdatesOpen(false)} />}
      {scanOpen && <ScanStatus agents={agents} onClose={() => setScanOpen(false)} />}
      {install && <InstallDialog agents={agents} preset={install} onClose={() => setInstall(null)} />}
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <MaintenanceProvider><Shell /></MaintenanceProvider>
    </QueryClientProvider>
  );
}
