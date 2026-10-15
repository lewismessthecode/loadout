import { useEffect, useMemo, useRef, useState } from "react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { Plus, RefreshCw, Search } from "lucide-react";
import { api } from "@/lib/api";
import { ALL, applyFilters, countAll, type Filter, type Sort, type SortKey } from "@/lib/filter";
import type { Skill } from "@/lib/types";
import { Sidebar } from "@/components/Sidebar";
import { SkillList } from "@/components/SkillList";
import { DetailPanel } from "@/components/DetailPanel";
import { InstallDialog, type InstallPreset } from "@/components/InstallDialog";
import { MiniHeat } from "@/components/Heatmap";
import { Button } from "@/components/ui";

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: true, retry: false } },
});

function filterTitle(f: Filter): string {
  switch (f.kind) {
    case "view":
      return { all: "All skills", recent: "Used recently", stale: "Unused for 30+ days", unknown: "Unknown source", plugins: "Plugins" }[f.view];
    case "agent":
      return f.id;
    case "source":
      return f.repo;
    case "tag":
      return `#${f.tag}`;
  }
}

function Shell() {
  const skillsQ = useQuery({ queryKey: ["skills"], queryFn: api.skills });
  const agentsQ = useQuery({ queryKey: ["agents"], queryFn: api.agents, staleTime: Infinity });
  const heatQ = useQuery({
    queryKey: ["heat"],
    queryFn: async () => Object.fromEntries((await api.heatmap(null, 365)).map((d) => [d.day, d.count])),
    enabled: skillsQ.isSuccess,
    placeholderData: (prev) => prev,
  });

  const [filter, setFilter] = useState<Filter>(ALL);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>({ key: "name", dir: "asc" });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [install, setInstall] = useState<InstallPreset | null>(null);
  const search = useRef<HTMLInputElement>(null);

  const skills = skillsQ.data ?? [];
  const agents = agentsQ.data ?? [];
  const counts = useMemo(() => countAll(skills, Date.now()), [skills]);
  const visible = useMemo(() => applyFilters(skills, filter, query, sort, Date.now()), [skills, filter, query, sort]);
  const selected: Skill | null = skills.find((s) => s.id === selectedId) ?? null;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
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
              {filter.kind === "agent" ? (agents.find((a) => a.id === filter.id)?.label ?? filter.id) : filterTitle(filter)}
            </h1>
          </div>
          <div className="relative">
            <Search size={13} className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-faint" />
            <input
              ref={search}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search"
              className="h-7 w-[190px] rounded-md border border-line bg-bg pr-2 pl-7 text-[13px] outline-none placeholder:text-faint focus:border-faint"
            />
          </div>
          <Button
            variant="ghost"
            aria-label="Rescan"
            title="Rescan skills and usage"
            onClick={() => {
              queryClient.invalidateQueries({ queryKey: ["skills"] });
              queryClient.invalidateQueries({ queryKey: ["heat"] });
            }}
            className="w-7 justify-center px-0"
          >
            <RefreshCw size={13} className={skillsQ.isFetching ? "animate-spin" : ""} />
          </Button>
          <Button variant="primary" onClick={() => setInstall({})}>
            <Plus size={13} /> Install
          </Button>
        </div>

        <div className="flex shrink-0 items-center gap-3 px-5 pt-1 pb-3 text-[12px] text-muted">
          <span>
            {visible.length} of {skills.length} skills · {used} used in logs
          </span>
          <span className="flex-1" />
          {heatQ.data && (
            <span className="flex items-center gap-2" title="Skill uses across all agents, last 26 weeks">
              <MiniHeat daily={heatQ.data} weeks={26} />
            </span>
          )}
        </div>

        {skillsQ.isLoading ? (
          <div className="grid flex-1 place-items-center text-muted">Scanning skills…</div>
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

      {install && <InstallDialog agents={agents} preset={install} onClose={() => setInstall(null)} />}
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Shell />
    </QueryClientProvider>
  );
}
