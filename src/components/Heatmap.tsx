import { useMemo } from "react";
import { buildGrid, monthLabels } from "@/lib/heat";
import { cn } from "@/lib/utils";

const plural = (n: number) => `${n} use${n === 1 ? "" : "s"}`;

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
  const grid = useMemo(() => buildGrid(daily, weeks), [daily, weeks]);
  const labels = useMemo(() => monthLabels(grid), [grid]);
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
            title={c.future ? undefined : `${c.date}: ${plural(c.count)}`}
            className={cn("aspect-square rounded-[2px]", c.future ? "opacity-0" : `heat-${c.level}`)}
          />
        ))}
      </div>
      <div className="mt-2 flex items-center justify-end gap-1 text-[10px] text-faint">
        Less
        {[0, 1, 2, 3, 4].map((l) => (
          <span key={l} className={cn("size-[9px] rounded-[2px]", `heat-${l}`)} />
        ))}
        More
      </div>
    </div>
  );
}
