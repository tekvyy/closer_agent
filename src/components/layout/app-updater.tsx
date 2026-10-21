"use client";

import { useEffect } from "react";
import { toast } from "sonner";
import { downloadAppUpdate, getJobsActive, installAppUpdate } from "@/lib/tauri/commands";

const FIRST_CHECK_DELAY_MS = 10_000;
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;
const ATTEMPTS = 3;

async function withRetries<T>(task: () => Promise<T>): Promise<T> {
  for (let attempt = 1; ; attempt++) {
    try {
      return await task();
    } catch (error) {
      if (attempt >= ATTEMPTS) throw error;
      console.warn(`Update attempt ${attempt} failed, retrying:`, error);
    }
  }
}

async function installAndRestart(version: string) {
  // Restarting kills running jobs, so let them finish first
  const activeJobs = await getJobsActive().catch(() => []);
  if (activeJobs.length > 0) {
    toast.info("Research is still running", {
      description: `Restart once the ${activeJobs.length === 1 ? "job finishes" : `${activeJobs.length} jobs finish`} to install the update.`,
    });
    offerRestart(version);
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

function offerRestart(version: string) {
  toast.info(`Closer ${version} is ready`, {
    id: "app-update",
    description: "Restart to finish updating. It only takes a few seconds.",
    duration: Infinity,
    action: { label: "Restart", onClick: () => void installAndRestart(version) },
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
    let checking = false;
    const checkForUpdate = async () => {
      if (downloaded || checking) return;
      checking = true;
      try {
        const version = await withRetries(downloadAppUpdate);
        if (!version) return;
        downloaded = true;
        offerRestart(version);
      } catch (error) {
        console.error("Update check failed:", error);
      } finally {
        checking = false;
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
