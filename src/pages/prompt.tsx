import { useQuery } from "@tanstack/react-query";
import { PromptEditor } from "@/components/prompt/prompt-editor";
import { IconTypography, IconLoader2 } from "@tabler/icons-react";
import { getPromptByType } from "@/lib/tauri/commands";
import type { PromptType } from "@/lib/tauri/types";

export type PromptContents = Record<PromptType, string>;

async function fetchAllPrompts(): Promise<PromptContents> {
  const types: PromptType[] = ["company", "person", "company_overview", "conversation_topics"];
  const results = await Promise.all(types.map((type) => getPromptByType(type)));

  return {
    company: results[0]?.content || "",
    person: results[1]?.content || "",
    company_overview: results[2]?.content || "",
    conversation_topics: results[3]?.content || "",
  };
}

export default function PromptPage() {
  const { data: prompts, isLoading } = useQuery({
    queryKey: ["prompts"],
    queryFn: fetchAllPrompts,
  });

  if (isLoading || !prompts) {
    return (
      <>
        <header className="h-10 border-b border-white/5 flex items-center px-4 gap-2">
          <IconTypography className="size-4" />
          <h1 className="text-sm font-medium">Prompt Configuration</h1>
        </header>
        <div className="flex items-center justify-center h-64">
          <IconLoader2 className="size-6 animate-spin text-muted-foreground" />
        </div>
      </>
    );
  }

  return (
    <>
      <header className="h-10 border-b border-white/5 flex items-center px-4 gap-2">
        <IconTypography className="size-4" />
        <h1 className="text-sm font-medium">Prompt Configuration</h1>
      </header>

      <PromptEditor key={JSON.stringify(prompts)} prompts={prompts} />
    </>
  );
}
