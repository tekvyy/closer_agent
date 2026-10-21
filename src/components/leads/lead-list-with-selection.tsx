import * as React from "react";
import { Link } from "react-router-dom";
import { IconChevronRight, IconSearch, IconTrash, IconChartBar } from "@tabler/icons-react";
import { SelectableEntityList, SelectableRow } from "@/components/selection";
import type { ActionConfig } from "@/components/selection";
import { ScoreBars } from "@/components/leads/score-bars";
import { ResearchStatusBadge } from "@/components/status/research-status-badge";
import { toast } from "sonner";
import { useSelectionStore } from "@/lib/store/selection-store";
import { deleteLeads, startResearch, startScoring } from "@/lib/tauri/commands";
import { handleStreamEvent } from "@/lib/stream/handle-stream-event";
import type { LeadScore } from "@/lib/tauri/types";
import {
  LEAD_USER_STATUS_ORDER,
  validateLeadUserStatus,
  LEAD_USER_STATUS_CONFIG,
} from "@/lib/constants/status-config";

type LeadWithScore = {
  id: number;
  companyName: string;
  city: string | null;
  state: string | null;
  website: string | null;
  industry: string | null;
  researchStatus: string | null;
  userStatus: string | null;
  score: LeadScore | null;
};

interface LeadListWithSelectionProps {
  groupedLeads: Record<string, LeadWithScore[]>;
  onRefresh?: () => void;
}

export function LeadListWithSelection({ groupedLeads, onRefresh }: LeadListWithSelectionProps) {
  const clearSelection = useSelectionStore((state) => state.clearAll);

  // Create a map for quick lead name lookups
  const leadMap = React.useMemo(() => {
    const map = new Map<number, LeadWithScore>();
    Object.values(groupedLeads)
      .flat()
      .forEach((lead) => map.set(lead.id, lead));
    return map;
  }, [groupedLeads]);

  const handleResearch = React.useCallback(
    async (selectedIds: number[]) => {
      const promises: Promise<boolean>[] = [];
      for (const leadId of selectedIds) {
        if (!leadMap.has(leadId)) continue;
        promises.push(
          (async () => {
            try {
              await startResearch(leadId, handleStreamEvent);
              return true;
            } catch (error) {
              console.error(`Failed to start research for lead ${leadId}:`, error);
              return false;
            }
          })()
        );
      }
      const results = await Promise.all(promises);

      let started = 0;
      let failed = 0;
      for (const ok of results) {
        if (ok) started++;
        else failed++;
      }

      if (started > 0) {
        toast.success(`Started research for ${started} lead${started > 1 ? "s" : ""}`);
      }
      if (failed > 0) {
        toast.error(`Failed to start research for ${failed} lead${failed > 1 ? "s" : ""}`);
      }
    },
    [leadMap]
  );

  const handleScore = React.useCallback(
    async (selectedIds: number[]) => {
      const promises: Promise<boolean>[] = [];
      for (const leadId of selectedIds) {
        if (!leadMap.has(leadId)) continue;
        promises.push(
          (async () => {
            try {
              await startScoring(leadId, handleStreamEvent);
              return true;
            } catch (error) {
              console.error(`Failed to start scoring for lead ${leadId}:`, error);
              return false;
            }
          })()
        );
      }
      const results = await Promise.all(promises);

      let started = 0;
      let failed = 0;
      for (const ok of results) {
        if (ok) started++;
        else failed++;
      }

      if (started > 0) {
        toast.success(`Started scoring for ${started} lead${started > 1 ? "s" : ""}`);
      }
      if (failed > 0) {
        toast.error(`Failed to start scoring for ${failed} lead${failed > 1 ? "s" : ""}`);
      }
    },
    [leadMap]
  );

  const handleDelete = React.useCallback(
    async (selectedIds: number[]) => {
      try {
        const deleted = await deleteLeads(selectedIds);
        clearSelection();
        onRefresh?.();
        toast.success(`Deleted ${deleted} lead${deleted > 1 ? "s" : ""}`);
      } catch (error) {
        console.error("Failed to delete leads:", error);
        toast.error("Failed to delete leads");
      }
    },
    [clearSelection, onRefresh]
  );

  const actions: ActionConfig[] = React.useMemo(
    () => [
      {
        id: "research",
        label: "Run Research",
        icon: IconSearch,
        group: "Research",
        onExecute: handleResearch,
      },
      {
        id: "score",
        label: "Score Leads",
        icon: IconChartBar,
        group: "Research",
        onExecute: handleScore,
      },
      {
        id: "delete",
        label: "Delete",
        icon: IconTrash,
        group: "Danger",
        destructive: true,
        onExecute: handleDelete,
      },
    ],
    [handleResearch, handleScore, handleDelete]
  );

  return (
    <SelectableEntityList
      entityType="lead"
      groupedItems={groupedLeads}
      statusOrder={LEAD_USER_STATUS_ORDER}
      configType="lead_user"
      getItemId={(lead) => lead.id}
      renderRow={(lead) => <LeadRow lead={lead} />}
      actions={actions}
    />
  );
}

function LeadRow({ lead }: { lead: LeadWithScore }) {
  const userStatus = validateLeadUserStatus(lead.userStatus);
  const userConfig = LEAD_USER_STATUS_CONFIG[userStatus];
  const StatusIcon = userConfig.icon;

  const location = [lead.city, lead.state].filter(Boolean).join(", ");
  const domain = lead.website?.replace(/^(https?:\/\/)?(www\.)?/, "").split("/")[0] || null;

  return (
    <SelectableRow id={lead.id}>
      <StatusIcon className={`size-4 ${userConfig.color} shrink-0`} />

      <Link to={`/lead/${lead.id}`} className="flex-1 min-w-0 flex items-center gap-1.5">
        {lead.companyName && (
          <>
            <span className="font-medium truncate">{lead.companyName}</span>
            <IconChevronRight className="size-3 text-muted-foreground shrink-0" />
          </>
        )}
        <span className="truncate text-muted-foreground">
          {location || "Unknown location"}
          {domain && <span className="text-muted-foreground/70"> - {domain}</span>}
        </span>
      </Link>

      <div className="flex items-center gap-2 shrink-0">
        {lead.industry && (
          <span className="text-xs px-1.5 py-0.5 rounded bg-white/5 text-muted-foreground">
            {lead.industry}
          </span>
        )}
        <ScoreBars score={lead.score} size="sm" />
        <ResearchStatusBadge status={lead.researchStatus} size="sm" />
      </div>
    </SelectableRow>
  );
}
