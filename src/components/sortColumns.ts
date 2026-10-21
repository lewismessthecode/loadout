export type SortCol = "installed" | "last" | "uses";

export const SORT_COL: Record<SortCol, { show: "always" | "xl" | "4xl"; width: string; align: "start" | "end" }> = {
  installed: {
    show: "4xl",
    width: "w-[84px] max-w-[84px] min-w-0 shrink-0 overflow-hidden",
    align: "start",
  },
  last: {
    show: "always",
    width: "w-[84px] max-w-[84px] min-w-0 shrink-0 overflow-hidden",
    align: "start",
  },
  uses: {
    show: "xl",
    width: "w-[64px] max-w-[64px] min-w-0 shrink-0 overflow-hidden",
    align: "end",
  },
};

export function sortHeadClass(col: SortCol): string {
  switch (col) {
    case "last":
      return `flex items-center gap-1 p-0 text-left ${SORT_COL.last.width}`;
    case "uses":
      return `hidden @xl:flex items-center justify-end gap-1 p-0 text-left ${SORT_COL.uses.width}`;
    case "installed":
      return `hidden @4xl:flex items-center gap-1 p-0 text-left ${SORT_COL.installed.width}`;
  }
}

export function metricCellClass(col: SortCol): string {
  switch (col) {
    case "last":
      return SORT_COL.last.width;
    case "uses":
      return `hidden @xl:block text-right ${SORT_COL.uses.width}`;
    case "installed":
      return `hidden @4xl:block ${SORT_COL.installed.width}`;
  }
}
