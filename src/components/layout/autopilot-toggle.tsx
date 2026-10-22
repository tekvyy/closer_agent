"use client";

import { IconBolt } from "@tabler/icons-react";
import { useSettingsStore } from "@/lib/store/settings-store";

/** Turns autopilot on and off: finished research starts the next step by itself */
export function AutopilotToggle() {
  const autoResearch = useSettingsStore((state) => state.autoResearch);
  const setAutoResearch = useSettingsStore((state) => state.setAutoResearch);
  const isInitialized = useSettingsStore((state) => state.isInitialized);

  return (
    <button
      onClick={() => setAutoResearch(!autoResearch)}
      disabled={!isInitialized}
      title="When research finishes, score the lead, research its people and write talking points automatically"
      className="flex items-center gap-2 w-full px-2 py-1 rounded text-muted-foreground hover:bg-white/5 hover:text-foreground transition-colors text-sm outline-none disabled:opacity-50"
    >
      <IconBolt className="size-3.5" />
      <span className="flex-1 text-left">Autopilot</span>
      <div
        className={`w-7 h-4 rounded-full transition-colors ${
          autoResearch ? "bg-primary" : "bg-white/20"
        } relative`}
      >
        <div
          className={`absolute top-0.5 size-3 rounded-full bg-white transition-transform ${
            autoResearch ? "translate-x-3.5" : "translate-x-0.5"
          }`}
        />
      </div>
    </button>
  );
}
