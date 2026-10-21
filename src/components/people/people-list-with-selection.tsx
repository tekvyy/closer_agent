import { Link } from "react-router-dom";
import { IconBuilding, IconTrash, IconSearch, IconMessage } from "@tabler/icons-react";
import { SelectableEntityList, SelectableRow } from "@/components/selection";
import type { ActionConfig } from "@/components/selection";
import { toast } from "sonner";
import { useSelectionStore } from "@/lib/store/selection-store";
import {
  deletePeople,
  startPersonResearch,
  startConversationGeneration,
} from "@/lib/tauri/commands";
import { handleStreamEvent } from "@/lib/stream/handle-stream-event";
import {
  PERSON_USER_STATUS_CONFIG,
  PERSON_USER_STATUS_ORDER,
  type PersonUserStatusType,
  validatePersonUserStatus,
} from "@/lib/constants/status-config";
import { ResearchStatusBadge } from "@/components/status/research-status-badge";
import { useMemo, useCallback } from "react";

type PersonWithCompany = {
  id: number;
  firstName: string;
  lastName: string;
  title: string | null;
  email: string | null;
  linkedinUrl: string | null;
  leadId: number | null;
  companyName: string | null;
  researchStatus: string | null;
  userStatus: string | null;
};

interface PeopleListWithSelectionProps {
  groupedPeople: Record<PersonUserStatusType, PersonWithCompany[]>;
  onRefresh?: () => void;
}

export function PeopleListWithSelection({
  groupedPeople,
  onRefresh,
}: PeopleListWithSelectionProps) {
  const clearSelection = useSelectionStore((state) => state.clearAll);

  // Create a map for quick person name lookups
  const personMap = useMemo(() => {
    const map = new Map<number, PersonWithCompany>();
    Object.values(groupedPeople)
      .flat()
      .forEach((person) => map.set(person.id, person));
    return map;
  }, [groupedPeople]);

  const handleResearch = useCallback(
    async (selectedIds: number[]) => {
      const promises: Promise<boolean>[] = [];
      for (const personId of selectedIds) {
        if (!personMap.has(personId)) continue;
        promises.push(
          (async () => {
            try {
              await startPersonResearch(personId, handleStreamEvent);
              return true;
            } catch (error) {
              console.error(`Failed to start research for person ${personId}:`, error);
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
        toast.success(`Started research for ${started} ${started > 1 ? "people" : "person"}`);
      }
      if (failed > 0) {
        toast.error(`Failed to start research for ${failed} ${failed > 1 ? "people" : "person"}`);
      }
    },
    [personMap]
  );

  const handleConversation = useCallback(
    async (selectedIds: number[]) => {
      const promises: Promise<boolean>[] = [];
      for (const personId of selectedIds) {
        if (!personMap.has(personId)) continue;
        promises.push(
          (async () => {
            try {
              await startConversationGeneration(personId, handleStreamEvent);
              return true;
            } catch (error) {
              console.error(
                `Failed to start conversation generation for person ${personId}:`,
                error
              );
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
        toast.success(
          `Started conversation generation for ${started} ${started > 1 ? "people" : "person"}`
        );
      }
      if (failed > 0) {
        toast.error(
          `Failed to start conversation generation for ${failed} ${failed > 1 ? "people" : "person"}`
        );
      }
    },
    [personMap]
  );

  const handleDelete = useCallback(
    async (selectedIds: number[]) => {
      try {
        const deleted = await deletePeople(selectedIds);
        clearSelection();
        onRefresh?.();
        toast.success(`Deleted ${deleted} ${deleted > 1 ? "people" : "person"}`);
      } catch (error) {
        console.error("Failed to delete people:", error);
        toast.error("Failed to delete people");
      }
    },
    [clearSelection, onRefresh]
  );

  const actions: ActionConfig[] = useMemo(
    () => [
      {
        id: "research",
        label: "Run Research",
        icon: IconSearch,
        group: "Research",
        onExecute: handleResearch,
      },
      {
        id: "conversation",
        label: "Generate Conversation",
        icon: IconMessage,
        group: "Research",
        onExecute: handleConversation,
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
    [handleResearch, handleConversation, handleDelete]
  );

  return (
    <SelectableEntityList
      entityType="person"
      groupedItems={groupedPeople}
      statusOrder={PERSON_USER_STATUS_ORDER}
      configType="person_user"
      getItemId={(person) => person.id}
      renderRow={(person) => <PersonRow person={person} />}
      actions={actions}
    />
  );
}

function PersonRow({ person }: { person: PersonWithCompany }) {
  const userStatus = validatePersonUserStatus(person.userStatus);
  const userConfig = PERSON_USER_STATUS_CONFIG[userStatus];
  const StatusIcon = userConfig.icon;

  const fullName = `${person.firstName} ${person.lastName}`;

  return (
    <SelectableRow id={person.id}>
      <StatusIcon className={`size-4 ${userConfig.color} shrink-0`} />

      <Link to={`/people/${person.id}`} className="flex-1 min-w-0 truncate">
        <span className="font-medium">{fullName}</span>
        {person.title && <span className="text-muted-foreground ml-2">{person.title}</span>}
      </Link>

      {person.leadId && person.companyName ? (
        <Link
          to={`/lead/${person.leadId}`}
          className="w-48 shrink-0 flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <IconBuilding className="size-3.5 shrink-0" />
          <span className="truncate">{person.companyName}</span>
        </Link>
      ) : (
        <div className="w-48 shrink-0 flex items-center gap-1.5 text-xs text-muted-foreground/50">
          <IconBuilding className="size-3.5 shrink-0" />
          <span className="truncate italic">No company</span>
        </div>
      )}

      <ResearchStatusBadge status={person.researchStatus} size="sm" />
    </SelectableRow>
  );
}
