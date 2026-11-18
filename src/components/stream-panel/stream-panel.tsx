"use client";

import { StreamPanelHeader } from "./stream-panel-header";
import { StreamPanelContent } from "./stream-panel-content";

export function StreamPanel() {
  return (
    <div className="bg-zinc-950 flex flex-col h-full">
      <StreamPanelHeader />
      <div className="flex-1 flex flex-col min-h-0">
        <StreamPanelContent />
      </div>
    </div>
  );
}
