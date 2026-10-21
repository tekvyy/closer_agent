"use client";

import { useEffect } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { toast } from "sonner";
import { getJobsActive } from "@/lib/tauri/commands";

const FIRST_CHECK_DELAY_MS = 10_000;
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;
const CHECK_TIMEOUT_MS = 30_000;
const DOWNLOAD_TIMEOUT_MS = 5 * 60 * 1000;
const ATTEMPTS = 4;

/**
 * Some networks block one of GitHub's download servers, and a connection to it hangs
 * instead of failing. Time out and retry: each attempt resolves the host again and
 * usually lands on a reachable server.
 */
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
    let checking = false;
    const checkForUpdate = async () => {
      if (downloaded || checking) return;
      checking = true;
      try {
        const update = await withRetries(() => check({ timeout: CHECK_TIMEOUT_MS }));
        if (!update) return;
        await withRetries(() => update.download(undefined, { timeout: DOWNLOAD_TIMEOUT_MS }));
        downloaded = true;
        offerRestart(update);
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
