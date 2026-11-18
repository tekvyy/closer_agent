"use client";

import { MarkdownRenderer } from "@/components/ui/markdown-renderer";
import { EmptyState } from "@/components/ui/empty-state";
import { useIsJobActive } from "@/lib/hooks/use-stream-tabs";
import { useJobSubmission } from "@/lib/hooks/use-job-submission";
import { IconPlayerPlay, IconFileText } from "@tabler/icons-react";
import { startPersonResearch } from "@/lib/tauri/commands";
import { handleStreamEvent } from "@/lib/stream/handle-stream-event";
import { toast } from "sonner";
import { wasSkipped } from "@/lib/research/start-for-each";

interface PersonResearchPanelProps {
  personId: number;
  personName: string;
  personProfile: string | null;
  companyName: string | null;
}

export function PersonResearchPanel({
  personId,
  personName,
  personProfile,
  companyName,
}: PersonResearchPanelProps) {
  const isJobActive = useIsJobActive(personId, "person");
  const { submit } = useJobSubmission();

  const handleStartResearch = async () => {
    await submit(async () => {
      // Start the research - backend will emit events
      // Event bridge handles tab creation and status updates
      // Logs stream directly via Channel callback for real-time display
      const result = await startPersonResearch(personId, handleStreamEvent);

      if (wasSkipped(result)) {
        toast.info(`${personName} already has an email`);
      } else {
        toast.success(`Looking for ${personName}'s email`);
      }
      return result;
    }).catch((error) => {
      console.error("Failed to start the email search:", error);
      toast.error("Couldn't start the email search");
    });
  };

  if (!personProfile) {
    const description = companyName
      ? `${companyName}'s website didn't give an email for ${personName}. A search for this person looks further.`
      : `Search the web for ${personName}'s email address.`;
    return (
      <EmptyState
        icon={IconFileText}
        title="No email yet"
        description={description}
        action={{
          label: "Find Email",
          loadingLabel: "Searching...",
          onClick: handleStartResearch,
          isLoading: isJobActive,
          icon: IconPlayerPlay,
        }}
      />
    );
  }

  return (
    <div className="min-h-[300px]">
      <MarkdownRenderer content={personProfile} />
    </div>
  );
}
