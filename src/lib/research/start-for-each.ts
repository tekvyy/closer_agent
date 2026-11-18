import { toast } from "sonner";
import { handleStreamEvent } from "@/lib/stream/handle-stream-event";
import type { StreamEvent } from "@/lib/tauri/types";

/** The backend answers "skipped" when there was nothing to do, e.g. the email is known */
export function wasSkipped(result: unknown): boolean {
  return (result as { status?: string } | null)?.status === "skipped";
}

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
        (result) => (wasSkipped(result) ? "skipped" : "started"),
        (error) => {
          console.error(`Failed to start ${what} for ${singular} ${id}:`, error);
          return "failed";
        }
      )
    )
  );

  const tally = (outcome: string) => results.filter((result) => result === outcome).length;
  const count = (n: number) => `${n} ${n === 1 ? singular : plural}`;
  if (tally("started") > 0) toast.success(`Started ${what} for ${count(tally("started"))}`);
  if (tally("skipped") > 0) {
    toast.info(`${count(tally("skipped"))} already had what ${what} looks for`);
  }
  if (tally("failed") > 0) toast.error(`Failed to start ${what} for ${count(tally("failed"))}`);
}
