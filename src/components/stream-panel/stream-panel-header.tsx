"use client";

import { useStreamPanelStore } from "@/lib/store/stream-panel-store";
import { useActiveJobs, useJob } from "@/lib/query/use-job-query";
import { cancelQueuedJobs } from "@/lib/tauri/commands";
import { toast } from "sonner";
import { StreamPanelTabs } from "./stream-panel-tabs";
import { useStreamSubscription } from "./use-stream-subscription";
import { IconChevronDown, IconChevronUp, IconPlayerStop } from "@tabler/icons-react";
import { Button } from "@/components/ui/button";

export function StreamPanelHeader() {
  const toggle = useStreamPanelStore((s) => s.toggle);
  const isOpen = useStreamPanelStore((s) => s.isOpen);
  const activeTabId = useStreamPanelStore((s) => s.activeTabId);

  const { killJob, closeTab } = useStreamSubscription();

  // Fetch active job details
  const { data: activeJob } = useJob(activeTabId ?? "", !!activeTabId);

  // Counted from every active job, not just the tabs (which show the newest 50)
  const { data: activeJobs = [] } = useActiveJobs();
  const runningCount = activeJobs.filter((job) => job.status === "running").length;
  const queuedCount = activeJobs.filter((job) => job.status === "queued").length;

  const handleStopQueued = async () => {
    try {
      const cancelled = await cancelQueuedJobs();
      toast.success(`Cancelled ${cancelled} queued job${cancelled === 1 ? "" : "s"}`, {
        description: "Jobs already running will finish.",
      });
    } catch (error) {
      toast.error("Couldn't cancel queued jobs", { description: String(error) });
    }
  };

  const handleCloseTab = async (jobId: string, isRunning: boolean) => {
    // Delete all job data when closing a tab
    await closeTab(jobId, isRunning);
  };

  const handleStopCurrent = async () => {
    if (!activeTabId) return;

    if (activeJob?.status === "running" || activeJob?.status === "queued") {
      await killJob(activeTabId);
    }
  };

  return (
    <div className="border-t border-white/10 bg-zinc-950 flex items-center justify-between border-b border-white/5 h-9 shrink-0">
      <StreamPanelTabs onCloseTab={handleCloseTab} />

      <div className="flex items-center gap-2 px-2">
        {(activeJob?.status === "running" || activeJob?.status === "queued") && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleStopCurrent}
            className="h-6 px-2 text-xs text-red-400 hover:text-red-300 hover:bg-red-500/10"
          >
            <IconPlayerStop className="size-3 mr-1" />
            Stop
          </Button>
        )}

        {queuedCount > 0 && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleStopQueued}
            title="Cancel every job that hasn't started yet; running jobs finish"
            className="h-6 px-2 text-xs text-muted-foreground hover:text-foreground"
          >
            <IconPlayerStop className="size-3 mr-1" />
            Stop queued
          </Button>
        )}

        {(runningCount > 0 || queuedCount > 0) && (
          <span className="flex items-center gap-1 text-xs text-muted-foreground">
            <span className="size-1.5 rounded-full bg-blue-400 animate-pulse" />
            {runningCount} running
            {queuedCount > 0 && ` · ${queuedCount} queued`}
          </span>
        )}

        <button
          onClick={toggle}
          aria-label={isOpen ? "Collapse panel" : "Expand panel"}
          className="p-1 rounded hover:bg-white/5 text-muted-foreground hover:text-foreground transition-colors"
        >
          {isOpen ? <IconChevronDown className="size-4" /> : <IconChevronUp className="size-4" />}
        </button>
      </div>
    </div>
  );
}
