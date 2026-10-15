import { useEffect, useRef, useState, type ReactNode } from "react";
import { Check, Clock, HelpCircle, Languages, Layers, Monitor, Moon, Puzzle, Sun, type LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { sameFilter, type Counts, type Filter, type View } from "@/lib/filter";
import { agentLabel } from "@/lib/format";
import { useI18n, type LangPref, type MessageKey } from "@/lib/i18n";
import type { AgentInfo } from "@/lib/types";
import { useTheme, type ThemePref } from "@/lib/theme";

const VIEWS: { view: View; label: MessageKey; icon: LucideIcon }[] = [
  { view: "all", label: "sidebar.views.all", icon: Layers },
  { view: "recent", label: "sidebar.views.recent", icon: Clock },
  { view: "stale", label: "sidebar.views.stale", icon: Moon },
  { view: "plugins", label: "sidebar.views.plugins", icon: Puzzle },
  { view: "unknown", label: "sidebar.views.unknown", icon: HelpCircle },
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

const THEMES: { pref: ThemePref; label: MessageKey; icon: LucideIcon }[] = [
  { pref: "system", label: "theme.system", icon: Monitor },
  { pref: "light", label: "theme.light", icon: Sun },
  { pref: "dark", label: "theme.dark", icon: Moon },
];

function ThemeSwitch() {
  const { t } = useI18n();
  const [pref, choose] = useTheme();
  return (
    <div role="radiogroup" aria-label={t("theme.label")} className="flex rounded-md bg-hover p-0.5">
      {THEMES.map(({ pref: p, label, icon: Icon }) => (
        <button
          key={p}
          role="radio"
          aria-checked={pref === p}
          aria-label={t(label)}
          title={t(label)}
          onClick={() => choose(p)}
          className={cn(
            "flex h-6 flex-1 items-center justify-center rounded text-muted",
            pref === p ? "bg-bg text-fg shadow-sm" : "hover:text-fg",
          )}
        >
          <Icon size={13} />
        </button>
      ))}
    </div>
  );
}

const LANG_OPTIONS: { pref: LangPref; label?: string }[] = [
  { pref: "system" },
  { pref: "en", label: "English" },
  { pref: "zh", label: "中文" },
];

function LangMenu() {
  const { pref, setPref, t } = useI18n();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={root} className="relative">
      <button
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t("lang.label")}
        title={t("lang.label")}
        onClick={() => setOpen((v) => !v)}
        className="flex size-7 items-center justify-center rounded-md bg-hover text-muted hover:text-fg"
      >
        <Languages size={14} />
      </button>
      {open && (
        <div
          role="menu"
          aria-label={t("lang.label")}
          className="absolute right-0 bottom-full z-50 mb-1.5 min-w-[140px] rounded-md border border-line bg-bg p-1 shadow-lg"
        >
          {LANG_OPTIONS.map(({ pref: p, label }) => (
            <button
              key={p}
              role="menuitemradio"
              aria-checked={pref === p}
              onClick={() => {
                setPref(p);
                setOpen(false);
              }}
              className={cn(
                "flex h-7 w-full items-center gap-2 rounded px-2 text-left text-[13px] hover:bg-hover",
                pref === p ? "font-medium text-fg" : "text-fg/80",
              )}
            >
              <span className="min-w-0 flex-1 truncate">{p === "system" ? t("lang.system") : label}</span>
              <span className="flex w-[13px] shrink-0 justify-center">{pref === p && <Check size={13} />}</span>
            </button>
          ))}
        </div>
      )}
    </div>
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
  const { t } = useI18n();
  const [allSources, setAllSources] = useState(false);
  const visibleAgents = agents.filter((a) => a.detected || (counts.agents[a.id] ?? 0) > 0);
  const sources = allSources ? counts.sources : counts.sources.slice(0, 8);

  return (
    <aside className="flex h-full w-[236px] shrink-0 flex-col border-r border-line bg-sidebar">
      <div data-tauri-drag-region className="h-[52px] shrink-0" />
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
              {t(label)}
            </Item>
          ))}
        </div>

        <Section title={t("common.agents")}>
          {visibleAgents.map((a) => (
            <Item
              key={a.id}
              active={sameFilter(filter, { kind: "agent", id: a.id })}
              count={counts.agents[a.id] ?? 0}
              onClick={() => onFilter({ kind: "agent", id: a.id })}
            >
              <span className={cn("size-1.5 shrink-0 rounded-full", a.detected ? "heat-3" : "bg-faint")} />
              {agentLabel(a.id, agents, t)}
            </Item>
          ))}
        </Section>

        {counts.sources.length > 0 && (
          <Section title={t("sidebar.sources")}>
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
                {allSources ? t("common.showLess") : t("sidebar.showMore", { n: counts.sources.length - 8 })}
              </button>
            )}
          </Section>
        )}

        {counts.tags.length > 0 && (
          <Section title={t("common.tags")}>
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
      <div className="flex shrink-0 items-center gap-1.5 border-t border-line p-2.5">
        <div className="min-w-0 flex-1">
          <ThemeSwitch />
        </div>
        <LangMenu />
      </div>
    </aside>
  );
}
