"use client";

import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { IconTrash, IconLoader2 } from "@tabler/icons-react";
import { deleteLeads } from "@/lib/tauri/commands";
import { useSelectionStore } from "@/lib/store/selection-store";
import { toast } from "sonner";

// SQLite caps bound parameters per statement, so delete in chunks.
const DELETE_CHUNK_SIZE = 500;

interface ClearLeadsModalProps {
  leadIds: number[];
  onSuccess?: () => void;
}

export function ClearLeadsModal({ leadIds, onSuccess }: ClearLeadsModalProps) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const clearSelection = useSelectionStore((state) => state.clearAll);

  const count = leadIds.length;

  const handleClear = async () => {
    setLoading(true);
    try {
      let deleted = 0;
      for (let i = 0; i < leadIds.length; i += DELETE_CHUNK_SIZE) {
        deleted += await deleteLeads(leadIds.slice(i, i + DELETE_CHUNK_SIZE));
      }
      clearSelection();
      setOpen(false);
      onSuccess?.();
      toast.success(`Cleared ${deleted} compan${deleted === 1 ? "y" : "ies"}`);
    } catch (error) {
      console.error("Failed to clear leads:", error);
      toast.error("Failed to clear companies");
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="ghost" size="sm" disabled={count === 0}>
          <IconTrash className="size-3.5" />
          Clear All
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Clear all companies?</DialogTitle>
          <DialogDescription>
            This permanently deletes all {count} compan{count === 1 ? "y" : "ies"}, along with their
            people and scores. This cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)} disabled={loading}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={handleClear} disabled={loading}>
            {loading && <IconLoader2 className="size-3.5 animate-spin" />}
            Delete {count} compan{count === 1 ? "y" : "ies"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
