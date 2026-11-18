"use client";

import { useStreamPanelStore } from "@/lib/store/stream-panel-store";
import { StreamPanel } from "./stream-panel";

/**
 * The page, with the activity log under it only while someone has opened the log from the
 * sidebar. Research running in the background shows in the sidebar instead, so the page
 * keeps its full height.
 */
export function StreamPanelWrapper({ children }: { children: React.ReactNode }) {
  const isOpen = useStreamPanelStore((s) => s.isOpen);

  return (
    <div className="flex-1 flex flex-col overflow-hidden min-w-0">
      <main className="flex-1 min-h-0 flex flex-col overflow-hidden">{children}</main>
      {isOpen && (
        <div className="h-[38%] min-h-48 shrink-0">
          <StreamPanel />
        </div>
      )}
    </div>
  );
}
