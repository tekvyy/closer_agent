import { useState } from "react";
import { IconLoader2, IconPlayerPlay } from "@tabler/icons-react";
import { Button } from "@/components/ui/button";
import { startForEach, type StartJob } from "@/lib/research/start-for-each";

interface ResearchAllButtonProps {
  /** Entities that haven't been researched yet */
  ids: number[];
  start: StartJob;
  /** Singular and plural, e.g. ["lead", "leads"] */
  noun: [string, string];
}

/** Starts research for every entity not researched yet, in one click */
export function ResearchAllButton({ ids, start, noun }: ResearchAllButtonProps) {
  const [isStarting, setIsStarting] = useState(false);

  if (ids.length === 0) return null;

  const handleClick = async () => {
    setIsStarting(true);
    try {
      await startForEach(ids, start, "research", noun);
    } finally {
      setIsStarting(false);
    }
  };

  return (
    <Button variant="outline" size="sm" onClick={handleClick} disabled={isStarting}>
      {isStarting ? (
        <IconLoader2 className="size-3.5 animate-spin" />
      ) : (
        <IconPlayerPlay className="size-3.5" />
      )}
      Research {ids.length} new {ids.length === 1 ? noun[0] : noun[1]}
    </Button>
  );
}
