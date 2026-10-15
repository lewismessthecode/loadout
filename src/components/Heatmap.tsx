import { useMemo } from "react";
import { buildGrid, monthLabels } from "@/lib/heat";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export function MiniHeat({ daily, weeks = 12 }: { daily: Record<string, number>; weeks?: number }) {
  const grid = useMemo(() => buildGrid(daily, weeks), [daily, weeks]);
  return (
    <div
      className="grid shrink-0 gap-px"
      style={{ gridAutoFlow: "column", gridTemplateRows: "repeat(7, 5px)", gridAutoColumns: "5px" }}
    >
      {grid.flat().map((c) => (
        <div
          key={c.date}
          className={cn("rounded-[1px]", c.future ? "opacity-0" : `heat-${c.level}`)}
        />
      ))}
    </div>
  );
}

export function FullHeat({ daily, weeks = 53 }: { daily: Record<string, number>; weeks?: number }) {
  const { lang, t } = useI18n();
  const grid = useMemo(() => buildGrid(daily, weeks), [daily, weeks]);
  const labels = useMemo(() => {
    const fmt = new Intl.DateTimeFormat(lang === "zh" ? "zh-CN" : "en", { month: "short" });
    return monthLabels(grid).map((l) => ({ col: l.col, label: fmt.format(new Date(2000, l.month, 1)) }));
  }, [grid, lang]);
  const uses = (n: number) => t(n === 1 ? "common.uses.one" : "common.uses.other", { n });
  return (
    <div>
      <div className="relative mb-1 h-3 text-[10px] text-faint">
        {labels.map((l) => (
          <span key={l.col} className="absolute" style={{ left: `${(l.col / weeks) * 100}%` }}>
            {l.label}
          </span>
        ))}
      </div>
      <div
        className="grid gap-[2px]"
        style={{ gridAutoFlow: "column", gridTemplateRows: "repeat(7, auto)", gridTemplateColumns: `repeat(${weeks}, minmax(0, 1fr))` }}
      >
        {grid.flat().map((c) => (
          <div
            key={c.date}
            title={c.future ? undefined : `${c.date}: ${uses(c.count)}`}
            className={cn("aspect-square rounded-[2px]", c.future ? "opacity-0" : `heat-${c.level}`)}
          />
        ))}
      </div>
      <div className="mt-2 flex items-center justify-end gap-1 text-[10px] text-faint">
        {t("heat.less")}
        {[0, 1, 2, 3, 4].map((l) => (
          <span key={l} className={cn("size-[9px] rounded-[2px]", `heat-${l}`)} />
        ))}
        {t("heat.more")}
      </div>
    </div>
  );
}
