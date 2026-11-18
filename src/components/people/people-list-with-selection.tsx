import { Link } from "react-router-dom";
import {
  IconBuilding,
  IconLoader2,
  IconMail,
  IconMailOff,
  IconTrash,
  IconSearch,
  IconMessage,
  IconSend,
} from "@tabler/icons-react";
import { useCrmStore } from "@/lib/store/crm-store";
import { SelectableEntityList, SelectableRow } from "@/components/selection";
import type { ActionConfig } from "@/components/selection";
import { toast } from "sonner";
import { useSelectionStore } from "@/lib/store/selection-store";
import {
  deletePeople,
  startPersonResearch,
  startConversationGeneration,
} from "@/lib/tauri/commands";
import { startForEach } from "@/lib/research/start-for-each";
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
  emailSource: "published" | "pattern" | null;
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
    (selectedIds: number[]) =>
      startForEach(
        selectedIds.filter((id) => personMap.has(id)),
        startPersonResearch,
        "the email search",
        ["person", "people"]
      ),
    [personMap]
  );

  const handleConversation = useCallback(
    (selectedIds: number[]) =>
      startForEach(
        selectedIds.filter((id) => personMap.has(id)),
        startConversationGeneration,
        "conversation generation",
        ["person", "people"]
      ),
    [personMap]
  );

  const pushToCrm = useCrmStore((state) => state.push);
  const handlePushToCrm = useCallback(
    async (selectedIds: number[]) => {
      const ids = selectedIds.filter((id) => personMap.has(id));
      if (await pushToCrm(ids)) clearSelection();
    },
    [personMap, pushToCrm, clearSelection]
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
        label: "Find Email",
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
        id: "push-to-crm",
        label: "Push to CRM",
        icon: IconSend,
        group: "CRM",
        onExecute: handlePushToCrm,
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
    [handleResearch, handleConversation, handlePushToCrm, handleDelete]
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

      <EmailCell
        email={person.email}
        emailSource={person.emailSource}
        researchStatus={person.researchStatus}
      />

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

/**
 * The person's email and how sure it is, or why there isn't one: the search ran and found
 * nothing, it is running, or company research found none and nobody has searched for this
 * person yet.
 */
function EmailCell({
  email,
  emailSource,
  researchStatus,
}: {
  email: string | null;
  emailSource: "published" | "pattern" | null;
  researchStatus: string | null;
}) {
  const cell = "w-60 shrink-0 flex items-center gap-1.5 text-xs";
  if (email) {
    const isPattern = emailSource === "pattern";
    return (
      <div
        title={
          isPattern
            ? `${email}\nWorked out from how this company writes its addresses, not seen published. It may bounce.`
            : email
        }
        className={`${cell} text-foreground select-text`}
      >
        <IconMail
          className={`size-3.5 shrink-0 ${isPattern ? "text-amber-500" : "text-green-500"}`}
        />
        <span className="truncate">{email}</span>
        {isPattern && (
          <span className="shrink-0 rounded bg-amber-500/15 px-1 text-[10px] text-amber-500 select-none">
            likely
          </span>
        )}
      </div>
    );
  }
  if (researchStatus === "in_progress") {
    return (
      <div title="Looking for their email" className={`${cell} text-muted-foreground`}>
        <IconLoader2 className="size-3.5 shrink-0 animate-spin" />
        <span className="truncate">Searching…</span>
      </div>
    );
  }
  if (researchStatus === "completed") {
    return (
      <div
        title="A search for this person found no email. Their page lists what was searched."
        className={`${cell} text-amber-500 font-medium`}
      >
        <IconMailOff className="size-3.5 shrink-0" />
        <span className="truncate">Email not found</span>
      </div>
    );
  }
  return (
    <div
      title={
        researchStatus === "failed"
          ? "The search failed. Run Find Email again."
          : "Their company's website didn't give an email. Find Email searches for this person."
      }
      className={`${cell} text-muted-foreground/60`}
    >
      <IconMail className="size-3.5 shrink-0" />
      <span className="truncate italic">
        {researchStatus === "failed" ? "Search failed" : "No email yet"}
      </span>
    </div>
  );
}
