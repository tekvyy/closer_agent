"use client";

import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { IconSearch, IconLoader2 } from "@tabler/icons-react";
import { startFindLeads } from "@/lib/tauri/commands";
import { handleStreamEvent } from "@/lib/stream/handle-stream-event";
import { toast } from "sonner";

interface FindLeadsModalProps {
  onSuccess?: () => void;
}

const LOCATION_KEY = "closer.find-leads.location";

/** The area typed last time, so it only has to be entered once */
function savedLocation(): string {
  try {
    return localStorage.getItem(LOCATION_KEY) ?? "";
  } catch {
    return "";
  }
}

/** What the search is given: who to look for, then where */
function describeSearch(who: string, where: string): string {
  const area = where.trim();
  return area ? `${who.trim()}\nLocation: ${area}` : who.trim();
}

export function FindLeadsModal({ onSuccess }: FindLeadsModalProps) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [icpDescription, setIcpDescription] = useState("");
  const [location, setLocation] = useState(savedLocation);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!icpDescription.trim()) return;

    setLoading(true);
    try {
      await startFindLeads(describeSearch(icpDescription, location), handleStreamEvent);
      try {
        localStorage.setItem(LOCATION_KEY, location.trim());
      } catch {
        // Remembering the area is a convenience; the search has started either way
      }
      setOpen(false);
      setIcpDescription("");
      onSuccess?.();
      toast.success("Search started", {
        description: "Companies appear here first, then their people and emails under People.",
      });
    } catch {
      toast.error("Couldn't start the search");
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          <IconSearch className="size-3.5" />
          Find Leads
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Find Leads</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="grid gap-4">
          <div className="grid gap-2">
            <Label htmlFor="icpDescription">Who are you looking for?</Label>
            <Textarea
              id="icpDescription"
              value={icpDescription}
              onChange={(e) => setIcpDescription(e.target.value)}
              placeholder="e.g. Nonprofit organizations with 20 to 200 staff"
              rows={3}
              required
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="findLeadsLocation">Where?</Label>
            <Input
              id="findLeadsLocation"
              value={location}
              onChange={(e) => setLocation(e.target.value)}
              placeholder="e.g. United States, or Atlanta, Georgia"
              autoComplete="off"
            />
            <p className="text-xs text-muted-foreground">
              A country or city keeps the results where you can work. Closer remembers it for next
              time.
            </p>
          </div>
          <p className="text-xs text-muted-foreground">
            Closer finds 10 to 20 companies, then looks up the right people at each one and their
            email addresses. Nothing more to click.
          </p>
          <DialogFooter>
            <Button type="submit" disabled={loading || !icpDescription.trim()}>
              {loading && <IconLoader2 className="size-3.5 animate-spin" />}
              {loading ? "Starting..." : "Find Leads"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
