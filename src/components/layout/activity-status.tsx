"use client";

import { useState } from "react";
import { IconLoader2, IconListDetails, IconPlayerStop } from "@tabler/icons-react";
import { toast } from "sonner";
import { useActiveJobs, useRecentJobs } from "@/lib/query/use-job-query";
import { useStreamPanelStore } from "@/lib/store/stream-panel-store";
import { cancelQueuedJobs, killJob } from "@/lib/tauri/commands";
import { queryClient } from "@/lib/query/query-client";
import { queryKeys } from "@/lib/query/keys";

/**
 * What research is doing right now, in the sidebar: how many jobs are running and waiting,
 * one Stop for all of them, and a way to open the step-by-step log. The log itself stays
 * closed unless asked for.
 */
export function ActivityStatus() {
  const { data: activeJobs = [] } = useActiveJobs();
  const { data: recentJobs = [] } = useRecentJobs(50);
  const isLogOpen = useStreamPanelStore((s) => s.isOpen);
  const toggleLog = useStreamPanelStore((s) => s.toggle);
  const [isStopping, setIsStopping] = useState(false);

  const running = activeJobs.filter((job) => job.status === "running");
  const waiting = activeJobs.filter((job) => job.status === "queued").length;
  const isBusy = running.length + waiting > 0;

  const handleStop = async () => {
    setIsStopping(true);
    try {
      // Waiting jobs first, so none of them takes a slot a stopped job frees
      await cancelQueuedJobs();
      await Promise.allSettled(running.map((job) => killJob(job.id)));
      toast.success("Research stopped", {
        description: "What was already found is kept.",
      });
    } catch (error) {
      toast.error("Couldn't stop research", { description: String(error) });
    } finally {
      queryClient.invalidateQueries({ queryKey: queryKeys.jobsActive() });
      queryClient.invalidateQueries({ queryKey: queryKeys.jobsRecent(50) });
      queryClient.invalidateQueries({ queryKey: queryKeys.leads });
      queryClient.invalidateQueries({ queryKey: queryKeys.people });
      setIsStopping(false);
    }
  };

  // Nothing has ever run, so there is no log to show either
  if (!isBusy && recentJobs.length === 0 && !isLogOpen) return null;

  return (
    <div className="p-2 border-t border-white/5 space-y-1">
      {isBusy && (
        <div className="flex items-center gap-2 px-2 py-1 text-sm" role="status">
          <IconLoader2 className="size-3.5 shrink-0 animate-spin text-blue-400" />
          <div className="flex-1 min-w-0">
            <div className="text-foreground">Researching…</div>
            <div className="text-xs text-muted-foreground">
              {running.length} running
              {waiting > 0 && ` · ${waiting} waiting`}
            </div>
          </div>
          <button
            onClick={handleStop}
            disabled={isStopping}
            title="Stop all research. What was already found is kept."
            className="flex items-center gap-1 px-1.5 py-0.5 rounded text-xs text-red-400 hover:text-red-300 hover:bg-red-500/10 transition-colors disabled:opacity-50"
          >
            <IconPlayerStop className="size-3" />
            Stop
          </button>
        </div>
      )}
      <button
        onClick={toggleLog}
        aria-pressed={isLogOpen}
        title="The step-by-step record of what research did, useful when something fails"
        className="flex items-center gap-2 w-full px-2 py-1 rounded text-muted-foreground hover:bg-white/5 hover:text-foreground transition-colors text-sm outline-none"
      >
        <IconListDetails className="size-3.5" />
        <span className="flex-1 text-left">{isLogOpen ? "Hide activity log" : "Activity log"}</span>
      </button>
    </div>
  );
}
