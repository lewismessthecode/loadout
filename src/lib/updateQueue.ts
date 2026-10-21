import type { UpdateOutcome } from "./types";

export interface UpdateRequest {
  skillId: string;
  planId: string;
}

export type UpdateProgress =
  | { status: "updating" }
  | { status: "updated"; outcome: UpdateOutcome }
  | { status: "failed"; message: string };

export function prepareUpdateProgress(previous: ReadonlyMap<string, UpdateProgress>, requests: UpdateRequest[]): Map<string, UpdateProgress> {
  const next = new Map(previous);
  for (const request of requests) next.delete(request.skillId);
  return next;
}

export async function runUpdateQueue({
  requests,
  apply,
  onProgress,
}: {
  requests: UpdateRequest[];
  apply: (request: UpdateRequest) => Promise<UpdateOutcome>;
  onProgress: (skillId: string, progress: UpdateProgress) => void;
}): Promise<void> {
  for (const request of requests) {
    onProgress(request.skillId, { status: "updating" });
    try {
      const outcome = await apply(request);
      onProgress(request.skillId, { status: "updated", outcome });
    } catch (error) {
      onProgress(request.skillId, { status: "failed", message: String(error) });
    }
  }
}
