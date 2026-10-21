import { createContext, useContext, useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./api";
import { invalidateCatalog } from "./reload";
import { runUpdateQueue, type UpdateProgress, type UpdateRequest } from "./updateQueue";
import type { UpdateInspection, UpdateOutcome, UpdateState } from "./types";

export type MutationKind = "install" | "remove" | "update";
interface Maintenance {
  states: ReadonlyMap<string, UpdateState>;
  outcomes: ReadonlyMap<string, UpdateOutcome>;
  checking: ReadonlySet<string>;
  busy: MutationKind | null;
  inspectionError: string | null;
  check: (ids: string[]) => Promise<void>;
  apply: (skillId: string, planId: string) => Promise<void>;
  applySelected: (requests: UpdateRequest[], onProgress: (skillId: string, progress: UpdateProgress) => void) => Promise<void>;
  run: <T>(kind: MutationKind, action: () => Promise<T>) => Promise<T>;
}

export function mergeInspections(previous: UpdateInspection[], incoming: UpdateInspection[]): UpdateInspection[] {
  const entries = new Map(previous.map((item) => [item.skillId, item]));
  for (const item of incoming) entries.set(item.skillId, item);
  return [...entries.values()];
}

export function mergeLocalInspections(previous: UpdateInspection[], incoming: UpdateInspection[]): UpdateInspection[] {
  const existing = new Map(previous.map((item) => [item.skillId, item]));
  return incoming.map((item) => item.state.status === "unchecked" ? existing.get(item.skillId) ?? item : item);
}

const Context = createContext<Maintenance | null>(null);

export function MaintenanceProvider({ children }: { children: ReactNode }) {
  const client = useQueryClient();
  const inspection = useQuery({
    queryKey: ["updates"],
    queryFn: async () => {
      const local = await api.inspectUpdates();
      return mergeLocalInspections(client.getQueryData<UpdateInspection[]>(["updates"]) ?? [], local);
    },
  });
  const [checking, setChecking] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<MutationKind | null>(null);
  const [outcomes, setOutcomes] = useState<Map<string, UpdateOutcome>>(new Map());
  const owner = useRef<MutationKind | null>(null);

  const run = async <T,>(kind: MutationKind, action: () => Promise<T>): Promise<T> => {
    if (owner.current) throw new Error("Another operation is in progress.");
    owner.current = kind;
    setBusy(kind);
    try {
      return await action();
    } finally {
      owner.current = null;
      setBusy(null);
    }
  };

  const check = async (ids: string[]) => {
    if (ids.length === 0) return;
    setChecking((current) => new Set([...current, ...ids]));
    try {
      const results = await api.checkUpdates(ids);
      client.setQueryData<UpdateInspection[]>(["updates"], (previous = []) => mergeInspections(previous, results));
    } catch (error) {
      const failed: UpdateInspection[] = ids.map((skillId) => ({ skillId, state: { status: "failed", message: String(error) } }));
      client.setQueryData<UpdateInspection[]>(["updates"], (previous = []) => mergeInspections(previous, failed));
    } finally {
      setChecking((current) => new Set([...current].filter((id) => !ids.includes(id))));
    }
  };

  const applyOne = async ({ skillId, planId }: UpdateRequest): Promise<UpdateOutcome> => {
    try {
      const outcome = await api.applyUpdate(planId);
      setOutcomes((current) => new Map(current).set(skillId, outcome));
      client.setQueryData<UpdateInspection[]>(["updates"], (previous = []) =>
        mergeInspections(previous, [{ skillId, state: { status: "unchecked" } }]),
      );
      return outcome;
    } catch (error) {
      client.setQueryData<UpdateInspection[]>(["updates"], (previous = []) =>
        mergeInspections(previous, [{ skillId, state: { status: "failed", message: String(error) } }]),
      );
      throw error;
    } finally {
      invalidateCatalog(client);
    }
  };

  const apply = async (skillId: string, planId: string) => {
    await run("update", () => applyOne({ skillId, planId }));
  };

  const applySelected = async (requests: UpdateRequest[], onProgress: (skillId: string, progress: UpdateProgress) => void) => {
    if (requests.length === 0) return;
    await run("update", () => runUpdateQueue({ requests, apply: applyOne, onProgress }));
  };

  return (
    <Context.Provider value={{
      states: new Map((inspection.data ?? []).map(({ skillId, state }) => [skillId, state])),
      outcomes, checking, busy, check, apply, applySelected, run,
      inspectionError: inspection.error ? String(inspection.error) : null,
    }}>
      {children}
    </Context.Provider>
  );
}

export function useMaintenance(): Maintenance {
  const value = useContext(Context);
  if (!value) throw new Error("MaintenanceProvider is required.");
  return value;
}
