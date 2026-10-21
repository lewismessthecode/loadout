import { describe, expect, it } from "vitest";
import type { QueryClient } from "@tanstack/react-query";
import { invalidateCatalog } from "./reload";

describe("invalidateCatalog", () => {
  it("marks the skill list, agent detection, and open detail stale", () => {
    const keys: unknown[] = [];
    const client = {
      invalidateQueries(filters: { queryKey: unknown }) {
        keys.push(filters.queryKey);
        return Promise.resolve();
      },
    } as unknown as QueryClient;

    invalidateCatalog(client);

    expect(keys).toEqual([["skills"], ["agents"], ["skill"], ["updates"]]);
  });
});