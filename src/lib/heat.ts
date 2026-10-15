export type Level = 0 | 1 | 2 | 3 | 4;

export interface Cell {
  date: string;
  count: number;
  level: Level;
  future: boolean;
}

export function level(count: number): Level {
  if (count <= 0) return 0;
  if (count < 2) return 1;
  if (count < 4) return 2;
  if (count < 7) return 3;
  return 4;
}

export function dateKey(d: Date): string {
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

/** Columns of 7 days (Sunday first). The last column contains `today`. */
export function buildGrid(daily: Record<string, number>, weeks: number, today = new Date()): Cell[][] {
  const end = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const start = new Date(end);
  start.setDate(end.getDate() - end.getDay() - (weeks - 1) * 7);
  const todayKey = dateKey(end);
  const cols: Cell[][] = [];
  for (let w = 0; w < weeks; w++) {
    const col: Cell[] = [];
    for (let d = 0; d < 7; d++) {
      const day = new Date(start);
      day.setDate(start.getDate() + w * 7 + d);
      const date = dateKey(day);
      const count = daily[date] ?? 0;
      col.push({ date, count, level: level(count), future: date > todayKey });
    }
    cols.push(col);
  }
  return cols;
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

export function monthLabels(grid: Cell[][]): { col: number; label: string }[] {
  const out: { col: number; label: string }[] = [];
  let prev = -1;
  grid.forEach((col, i) => {
    const month = Number(col[0].date.slice(5, 7)) - 1;
    if (month !== prev) {
      if (prev !== -1 || col[0].date.slice(8) <= "07") out.push({ col: i, label: MONTHS[month] });
      prev = month;
    }
  });
  return out.filter((l, i, arr) => i === 0 || l.col - arr[i - 1].col >= 3);
}
