import { toast } from "sonner";
import { handleStreamEvent } from "@/lib/stream/handle-stream-event";
import type { StreamEvent } from "@/lib/tauri/types";

export type StartJob = (id: number, onEvent: (event: StreamEvent) => void) => Promise<unknown>;

/**
 * Starts a job for each id and toasts how many started. Jobs beyond the concurrency
 * limit wait in the queue, so any number of ids is fine.
 *
 * @param what the job, as in "Started {what} for 3 leads"
 * @param noun singular and plural of the entity, e.g. ["lead", "leads"]
 */
export async function startForEach(
  ids: number[],
  start: StartJob,
  what: string,
  [singular, plural]: [string, string]
): Promise<void> {
  const results = await Promise.all(
    ids.map((id) =>
      start(id, handleStreamEvent).then(
        () => true,
        (error) => {
          console.error(`Failed to start ${what} for ${singular} ${id}:`, error);
          return false;
        }
      )
    )
  );

  const started = results.filter(Boolean).length;
  const failed = results.length - started;
  const count = (n: number) => `${n} ${n === 1 ? singular : plural}`;
  if (started > 0) toast.success(`Started ${what} for ${count(started)}`);
  if (failed > 0) toast.error(`Failed to start ${what} for ${count(failed)}`);
}
