"use client";

import { useEffect, useState } from "react";
import { toast } from "sonner";
import { getVersion } from "@tauri-apps/api/app";
import { IconDownload, IconLoader2 } from "@tabler/icons-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { getJobsActive, installAppUpdate } from "@/lib/tauri/commands";
import { useUpdateStore } from "@/lib/store/update-store";

const FIRST_CHECK_DELAY_MS = 10_000;
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;

async function installAndRestart(version: string) {
  // Restarting kills running jobs, so let them finish first
  const activeJobs = await getJobsActive().catch(() => []);
  if (activeJobs.length > 0) {
    toast.info("Research is still running", {
      description: `Restart once the ${activeJobs.length === 1 ? "job finishes" : `${activeJobs.length} jobs finish`} to install Closer ${version}. Use Install in the sidebar when it's done.`,
    });
    return;
  }

  const installing = toast.loading(`Installing Closer ${version}…`);
  try {
    // Restarts into the new version; on Windows the installer closes and reopens the app
    await installAppUpdate();
  } catch (error) {
    toast.error("The update couldn't be installed", {
      id: installing,
      description: String(error),
    });
  }
}

/**
 * Checks for a new app version at startup and every few hours, downloads it in the
 * background, then asks in a dialog to restart into it.
 */
export function AppUpdater() {
  const status = useUpdateStore((state) => state.status);
  const dialogOpen = useUpdateStore((state) => state.dialogOpen);
  const setDialogOpen = useUpdateStore((state) => state.setDialogOpen);
  const check = useUpdateStore((state) => state.check);

  useEffect(() => {
    if (import.meta.env.DEV) return;
    const firstCheck = setTimeout(() => void check(false), FIRST_CHECK_DELAY_MS);
    const interval = setInterval(() => void check(false), CHECK_INTERVAL_MS);
    return () => {
      clearTimeout(firstCheck);
      clearInterval(interval);
    };
  }, [check]);

  if (status.kind !== "ready") return null;
  const { version } = status;

  return (
    <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Closer {version} is ready</DialogTitle>
          <DialogDescription>
            A new version has been downloaded. Restart to install it. It takes a few seconds, and
            your leads and people are kept.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => setDialogOpen(false)}>
            Later
          </Button>
          <Button
            onClick={() => {
              setDialogOpen(false);
              void installAndRestart(version);
            }}
          >
            Restart now
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** The app version in the sidebar's settings, with a way to check for or install an update. */
export function AppUpdateRow() {
  const status = useUpdateStore((state) => state.status);
  const check = useUpdateStore((state) => state.check);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  const label =
    status.kind === "checking"
      ? "Checking…"
      : status.kind === "ready"
        ? `Install ${status.version}`
        : status.kind === "failed"
          ? "Retry"
          : status.kind === "up_to_date"
            ? "Up to date"
            : "Check";

  const tone =
    status.kind === "ready" ? "text-green-500" : status.kind === "failed" ? "text-destructive" : "";

  return (
    <button
      onClick={() => void check(true)}
      disabled={status.kind === "checking"}
      title={
        status.kind === "failed"
          ? `The last update check failed: ${status.message}`
          : "Check for a newer version of Closer"
      }
      className="flex items-center gap-2 w-full px-2 py-1 rounded text-muted-foreground hover:bg-white/5 hover:text-foreground transition-colors text-sm outline-none disabled:opacity-70"
    >
      {status.kind === "checking" ? (
        <IconLoader2 className="size-3.5 animate-spin" />
      ) : (
        <IconDownload className="size-3.5" />
      )}
      <span className="flex-1 text-left">{version ? `Version ${version}` : "Updates"}</span>
      <span className={`text-xs ${tone}`}>{label}</span>
    </button>
  );
}
