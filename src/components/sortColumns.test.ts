import { describe, expect, it } from "vitest";
import { SORT_COL, metricCellClass, sortHeadClass, type SortCol } from "./sortColumns";

const width = {
  installed: "w-[84px] max-w-[84px] min-w-0 shrink-0 overflow-hidden",
  last: "w-[84px] max-w-[84px] min-w-0 shrink-0 overflow-hidden",
  uses: "w-[64px] max-w-[64px] min-w-0 shrink-0 overflow-hidden",
};

describe("sort columns", () => {
  it("keeps the arrow inside a flex header and aligns the row to the same width", () => {
    expect(SORT_COL.last.width).toBe(width.last);
    expect(SORT_COL.installed.width).toBe(width.installed);
    expect(SORT_COL.uses.width).toBe(width.uses);

    expect(sortHeadClass("last")).toBe(`flex items-center gap-1 p-0 text-left ${SORT_COL.last.width}`);
    expect(sortHeadClass("uses")).toBe(`hidden @xl:flex items-center justify-end gap-1 p-0 text-left ${SORT_COL.uses.width}`);
    expect(sortHeadClass("installed")).toBe(`hidden @4xl:flex items-center gap-1 p-0 text-left ${SORT_COL.installed.width}`);

    expect(metricCellClass("last")).toBe(SORT_COL.last.width);
    expect(metricCellClass("uses")).toBe(`hidden @xl:block text-right ${SORT_COL.uses.width}`);
    expect(metricCellClass("installed")).toBe(`hidden @4xl:block ${SORT_COL.installed.width}`);

    const cols: SortCol[] = ["last", "uses", "installed"];
    for (const col of cols) {
      expect(sortHeadClass(col)).not.toMatch(/block/);
    }
    expect(sortHeadClass("uses").split(/\s+/)).not.toContain("flex");
    expect(sortHeadClass("installed").split(/\s+/)).not.toContain("flex");
    expect(sortHeadClass("last").split(/\s+/)).toContain("flex");
  });
});
