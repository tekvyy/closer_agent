"use client";

import { useState, ReactNode } from "react";
import { IconChevronDown, IconChevronRight, IconPlus } from "@tabler/icons-react";
import { cn } from "@/lib/utils";
import {
  RESEARCH_STATUS_CONFIG,
  LEAD_USER_STATUS_CONFIG,
  PERSON_USER_STATUS_CONFIG,
  type ResearchStatusType,
  type LeadUserStatusType,
  type PersonUserStatusType,
} from "@/lib/constants/status-config";

type StatusConfigType = "research" | "lead_user" | "person_user";

interface CollapsibleStatusGroupProps {
  status: string;
  count: number;
  children: ReactNode;
  startsOpen?: boolean;
  configType?: StatusConfigType;
}

export function CollapsibleStatusGroup({
  status,
  count,
  children,
  startsOpen = true,
  configType = "research",
}: CollapsibleStatusGroupProps) {
  const [isOpen, setIsOpen] = useState(() => startsOpen);

  // Get the appropriate config based on type
  const config =
    configType === "lead_user"
      ? LEAD_USER_STATUS_CONFIG[status as LeadUserStatusType]
      : configType === "person_user"
        ? PERSON_USER_STATUS_CONFIG[status as PersonUserStatusType]
        : RESEARCH_STATUS_CONFIG[status as ResearchStatusType];

  const StatusIcon = config.icon;

  return (
    <div className="group/status">
      <div className="sticky top-0 bg-black/95 backdrop-blur-sm z-10 flex items-center gap-2 px-3 py-2 text-sm border-b border-white/5">
        <button
          onClick={() => setIsOpen(!isOpen)}
          className="w-4 shrink-0 flex items-center justify-center hover:bg-white/10 transition-colors"
        >
          {isOpen ? (
            <IconChevronDown className="size-3 text-muted-foreground" />
          ) : (
            <IconChevronRight className="size-3 text-muted-foreground" />
          )}
        </button>
        <StatusIcon className={cn("size-4 shrink-0", config.color)} />
        <span className="font-medium">{config.label}</span>
        <span className="text-muted-foreground text-xs">{count}</span>
        <div className="flex-1" />
        <button className="p-1 hover:bg-white/10 opacity-0 group-hover/status:opacity-100 transition-opacity">
          <IconPlus className="size-3.5 text-muted-foreground" />
        </button>
      </div>

      {isOpen && children}
    </div>
  );
}
