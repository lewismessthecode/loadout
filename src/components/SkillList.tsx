import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp } from "lucide-react";
import { cn } from "@/lib/utils";
import { relTime, shortDate } from "@/lib/format";
import type { Sort, SortKey } from "@/lib/filter";
import type { AgentInfo, Skill } from "@/lib/types";
import { Chip } from "./ui";
import { MiniHeat } from "./Heatmap";

const ROW_H = 58;

function SortHead({
  label,
  k,
  sort,
  onSort,
  className,
}: {
  label: string;
  k: SortKey;
  sort: Sort;
  onSort: (k: SortKey) => void;
  className?: string;
}) {
  const active = sort.key === k;
  return (
    <button
      onClick={() => onSort(k)}
      className={cn("flex items-center gap-1 text-left hover:text-fg", active && "text-fg", className)}
    >
      {label}
      {active && (sort.dir === "asc" ? <ArrowUp size={11} /> : <ArrowDown size={11} />)}
    </button>
  );
}

const COLS = {
  agents: "hidden w-[112px] shrink-0 @xl:flex",
  source: "hidden w-[150px] shrink-0 @3xl:block",
  installed: "hidden w-[84px] shrink-0 @4xl:block",
  last: "w-[84px] shrink-0",
  uses: "hidden w-[40px] shrink-0 text-right @xl:block",
  heat: "hidden w-[76px] shrink-0 @2xl:block",
};

export function SkillList({
  skills,
  agents,
  selectedId,
  onSelect,
  sort,
  onSort,
}: {
  skills: Skill[];
  agents: AgentInfo[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  sort: Sort;
  onSort: (k: SortKey) => void;
}) {
  const parent = useRef<HTMLDivElement>(null);
  const virt = useVirtualizer({
    count: skills.length,
    getScrollElement: () => parent.current,
    estimateSize: () => ROW_H,
    overscan: 8,
  });
  const labels = Object.fromEntries(agents.map((a) => [a.id, a.label]));
  const now = Date.now();

  return (
    <div className="@container flex min-h-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-4 border-b border-line px-5 text-[11px] font-medium text-faint">
        <SortHead label="Skill" k="name" sort={sort} onSort={onSort} className="min-w-0 flex-1" />
        <div className={COLS.agents}>Agents</div>
        <div className={COLS.source}>Source</div>
        <SortHead label="Installed" k="installed" sort={sort} onSort={onSort} className={cn(COLS.installed)} />
        <SortHead label="Last used" k="lastUsed" sort={sort} onSort={onSort} className={COLS.last} />
        <SortHead label="Uses" k="total" sort={sort} onSort={onSort} className={cn(COLS.uses, "justify-end")} />
        <div className={COLS.heat}>12 weeks</div>
      </div>

      <div ref={parent} className="min-h-0 flex-1 overflow-y-auto">
        {skills.length === 0 ? (
          <div className="grid h-40 place-items-center text-muted">No skills match.</div>
        ) : (
          <div style={{ height: virt.getTotalSize(), position: "relative" }}>
            {virt.getVirtualItems().map((v) => {
              const s = skills[v.index];
              const agentIds = [...new Set(s.installs.map((i) => i.agent))];
              return (
                <div
                  key={s.id}
                  onClick={() => onSelect(s.id)}
                  style={{ position: "absolute", top: 0, left: 0, right: 0, height: v.size, transform: `translateY(${v.start}px)` }}
                  className={cn(
                    "flex items-center gap-4 border-b border-line/60 px-5",
                    s.id === selectedId ? "bg-selected" : "hover:bg-hover",
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium">{s.name}</div>
                    <div className="truncate text-[12px] text-muted">{s.description || "No description"}</div>
                  </div>
                  <div className={cn(COLS.agents, "flex-wrap items-center gap-1")}>
                    {agentIds.slice(0, 2).map((a) => (
                      <Chip key={a}>{shortLabel(labels[a] ?? a)}</Chip>
                    ))}
                    {agentIds.length > 2 && <span className="text-[11px] text-faint">+{agentIds.length - 2}</span>}
                  </div>
                  <div className={cn(COLS.source, "truncate text-[12px] text-muted")} title={s.source?.repo}>
                    {s.source?.repo ?? <span className="text-faint">local</span>}
                  </div>
                  <div className={cn(COLS.installed, "text-[12px] text-muted tabular-nums")}>{shortDate(s.installedAt)}</div>
                  <div className={cn(COLS.last, "text-[12px] tabular-nums", s.usage.lastUsed ? "text-fg/80" : "text-faint")}>
                    {relTime(s.usage.lastUsed, now)}
                  </div>
                  <div className={cn(COLS.uses, "text-[12px] text-muted tabular-nums")}>{s.usage.total || ""}</div>
                  <div className={COLS.heat}>
                    <MiniHeat daily={s.usage.daily} />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function shortLabel(label: string): string {
  return label.replace(/ \(.*\)$/, "").replace(" Code", "").replace(" CLI", "");
}
