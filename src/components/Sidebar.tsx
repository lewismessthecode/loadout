import { useState, type ReactNode } from "react";
import { Boxes, Clock, HelpCircle, Layers, Moon, Puzzle, type LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { sameFilter, type Counts, type Filter, type View } from "@/lib/filter";
import type { AgentInfo } from "@/lib/types";

const VIEWS: { view: View; label: string; icon: LucideIcon }[] = [
  { view: "all", label: "All skills", icon: Layers },
  { view: "recent", label: "Used recently", icon: Clock },
  { view: "stale", label: "Unused 30d+", icon: Moon },
  { view: "plugins", label: "Plugins", icon: Puzzle },
  { view: "unknown", label: "Unknown source", icon: HelpCircle },
];

function Item({
  active,
  count,
  onClick,
  children,
  title,
}: {
  active: boolean;
  count: number;
  onClick: () => void;
  children: ReactNode;
  title?: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={cn(
        "flex h-7 w-full items-center gap-2 rounded-md px-2 text-left text-[13px]",
        active ? "bg-selected font-medium" : "text-fg/80 hover:bg-hover",
      )}
    >
      <span className="flex min-w-0 flex-1 items-center gap-2 truncate">{children}</span>
      <span className="text-[11px] tabular-nums text-faint">{count}</span>
    </button>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="mt-5">
      <div className="px-2 pb-1 text-[11px] font-medium tracking-wide text-faint uppercase">{title}</div>
      {children}
    </div>
  );
}

export function Sidebar({
  filter,
  onFilter,
  counts,
  agents,
}: {
  filter: Filter;
  onFilter: (f: Filter) => void;
  counts: Counts;
  agents: AgentInfo[];
}) {
  const [allSources, setAllSources] = useState(false);
  const visibleAgents = agents.filter((a) => a.detected || (counts.agents[a.id] ?? 0) > 0);
  const sources = allSources ? counts.sources : counts.sources.slice(0, 8);

  return (
    <aside className="flex h-full w-[236px] shrink-0 flex-col border-r border-line bg-sidebar">
      <div data-tauri-drag-region className="flex h-[52px] shrink-0 items-end pb-2 pl-[78px]">
        <div className="pointer-events-none flex items-center gap-1.5 text-[13px] font-semibold">
          <Boxes size={14} />
          SkillsHub
        </div>
      </div>
      <nav className="flex-1 overflow-y-auto px-2 pb-4">
        <div>
          {VIEWS.map(({ view, label, icon: Icon }) => (
            <Item
              key={view}
              active={sameFilter(filter, { kind: "view", view })}
              count={counts.views[view]}
              onClick={() => onFilter({ kind: "view", view })}
            >
              <Icon size={14} className="shrink-0 text-muted" />
              {label}
            </Item>
          ))}
        </div>

        <Section title="Agents">
          {visibleAgents.map((a) => (
            <Item
              key={a.id}
              active={sameFilter(filter, { kind: "agent", id: a.id })}
              count={counts.agents[a.id] ?? 0}
              onClick={() => onFilter({ kind: "agent", id: a.id })}
            >
              <span className={cn("size-1.5 shrink-0 rounded-full", a.detected ? "heat-3" : "bg-faint")} />
              {a.label}
            </Item>
          ))}
        </Section>

        {counts.sources.length > 0 && (
          <Section title="Sources">
            {sources.map(([repo, n]) => (
              <Item
                key={repo}
                title={repo}
                active={sameFilter(filter, { kind: "source", repo })}
                count={n}
                onClick={() => onFilter({ kind: "source", repo })}
              >
                {repo}
              </Item>
            ))}
            {counts.sources.length > 8 && (
              <button
                onClick={() => setAllSources((v) => !v)}
                className="mt-0.5 px-2 text-[12px] text-muted hover:text-fg"
              >
                {allSources ? "Show less" : `Show ${counts.sources.length - 8} more`}
              </button>
            )}
          </Section>
        )}

        {counts.tags.length > 0 && (
          <Section title="Tags">
            {counts.tags.map(([tag, n]) => (
              <Item
                key={tag}
                active={sameFilter(filter, { kind: "tag", tag })}
                count={n}
                onClick={() => onFilter({ kind: "tag", tag })}
              >
                <span className="text-faint">#</span>
                {tag}
              </Item>
            ))}
          </Section>
        )}
      </nav>
    </aside>
  );
}
