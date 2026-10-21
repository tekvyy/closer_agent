"use client";

import { useEffect } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { toast } from "sonner";
import { getJobsActive } from "@/lib/tauri/commands";

const FIRST_CHECK_DELAY_MS = 10_000;
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;

async function installAndRestart(update: Update) {
  // Restarting kills running jobs, so let them finish first
  const activeJobs = await getJobsActive().catch(() => []);
  if (activeJobs.length > 0) {
    toast.info("Research is still running", {
      description: `Restart once the ${activeJobs.length === 1 ? "job finishes" : `${activeJobs.length} jobs finish`} to install the update.`,
    });
    offerRestart(update);
    return;
  }

  const installing = toast.loading(`Installing Closer ${update.version}…`);
  try {
    // On Windows this closes the app and the installer reopens it
    await update.install();
    await relaunch();
  } catch (error) {
    toast.error("The update couldn't be installed", {
      id: installing,
      description: String(error),
    });
  }
}

function offerRestart(update: Update) {
  toast.info(`Closer ${update.version} is ready`, {
    id: "app-update",
    description: "Restart to finish updating. It only takes a few seconds.",
    duration: Infinity,
    action: { label: "Restart", onClick: () => void installAndRestart(update) },
  });
}

/**
 * Checks for a new app version at startup and every few hours, downloads it in the
 * background, then offers a one-click restart to install it.
 */
export function AppUpdater() {
  useEffect(() => {
    if (import.meta.env.DEV) return;

    let downloaded = false;
    const checkForUpdate = async () => {
      if (downloaded) return;
      try {
        const update = await check();
        if (!update) return;
        await update.download();
        downloaded = true;
        offerRestart(update);
      } catch (error) {
        console.error("Update check failed:", error);
      }
    };

    const firstCheck = setTimeout(checkForUpdate, FIRST_CHECK_DELAY_MS);
    const interval = setInterval(checkForUpdate, CHECK_INTERVAL_MS);
    return () => {
      clearTimeout(firstCheck);
      clearInterval(interval);
    };
  }, []);

  return null;
}
