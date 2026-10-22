"use client";

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { IconLoader2 } from "@tabler/icons-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import {
  installCli,
  openCliTerminal,
  type CliAction,
  type CliInstallOutput,
  type CliIssue,
} from "@/lib/tauri/commands";
import { useSettingsStore, providerOf, CLI_NAMES, ACCOUNT_NAMES } from "@/lib/store/settings-store";

interface IssueCopy {
  title: string;
  description: string;
  action?: { type: CliAction; label: string };
}

function describeIssue({ provider, kind }: CliIssue, version: string | null): IssueCopy {
  const cli = CLI_NAMES[provider];
  const account = ACCOUNT_NAMES[provider];
  const installed = version ? `${cli} ${version}` : cli;

  switch (kind) {
    case "missing":
      return {
        title: `Set up ${cli}`,
        description: `Closer runs ${account} models through the ${cli} CLI, which isn't installed on this computer yet. Click Install and Closer downloads and sets it up for you. Afterwards, you sign in with your ${account} account.`,
        action: { type: "install", label: "Install" },
      };
    case "outdated":
      return {
        title: `${cli} couldn't use this model`,
        description: `${installed} rejected the selected model. Updating ${cli} usually fixes this: click Update, then run the job again. If you already have the newest version, your ${account} account may not have this model; pick another one in the sidebar.`,
        action: { type: "update", label: "Update" },
      };
    case "update":
      return {
        title: `Update ${cli}`,
        description: `${version ? `You have ${cli} ${version}. ` : ""}Click Update to install the newest version. If you already have it, nothing changes.`,
        action: { type: "update", label: "Update" },
      };
    case "login":
      return {
        title: `Sign in to ${cli}`,
        description: `${cli} isn't signed in yet. Click Sign in: a small window opens and your browser asks you to log in with your ${account} account. When the browser says you're signed in, come back here and click Check again.`,
        action: { type: "login", label: "Sign in" },
      };
    case "limit":
      return {
        title: `${account} usage limit reached`,
        description: `Your ${account} account hit its usage limit. Wait for it to reset, or pick a model from the other provider in the sidebar.`,
      };
  }
}

/**
 * Shows fixable agent CLI problems (missing, outdated, signed out, usage limit) reported
 * by failed jobs, and checks the CLI up front whenever the selected provider changes.
 * Installs and updates run in the background here; sign-in opens a terminal window.
 */
export function CliIssueDialog() {
  const issue = useSettingsStore((state) => state.cliIssue);
  const setIssue = useSettingsStore((state) => state.setCliIssue);
  const [isOpening, setIsOpening] = useState(false);
  const [isChecking, setIsChecking] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);
  const [installLog, setInstallLog] = useState<string[]>([]);
  const [installError, setInstallError] = useState<string | null>(null);

  const isInitialized = useSettingsStore((state) => state.isInitialized);
  const provider = useSettingsStore((state) => providerOf(state.selectedModel));
  const refreshCliStatus = useSettingsStore((state) => state.refreshCliStatus);
  const cliStatus = useSettingsStore((state) => state.cliStatus);

  // Failed jobs report fixable CLI problems through this event
  useEffect(() => {
    const unlisten = listen<CliIssue>("cli-issue", (event) => setIssue(event.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setIssue]);

  useEffect(() => {
    const unlisten = listen<CliInstallOutput>("cli-install-output", (event) =>
      setInstallLog((log) => [...log.slice(-5), event.payload.line])
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // A new issue starts without the previous install's output, unless that install is
  // still running (the dialog was closed and reopened mid-install)
  const issueKey = issue ? `${issue.provider}:${issue.kind}` : null;
  const [shownIssueKey, setShownIssueKey] = useState(issueKey);
  if (issueKey !== shownIssueKey) {
    setShownIssueKey(issueKey);
    if (!isInstalling) {
      setInstallLog([]);
      setInstallError(null);
    }
  }

  // Check both CLIs at startup and on provider change. A missing CLI switches to the
  // other provider (see refreshCliStatus); the dialog covers having no CLI at all, or
  // the selected one being signed out.
  useEffect(() => {
    if (!isInitialized) return;
    let cancelled = false;
    refreshCliStatus().then(() => {
      if (cancelled) return;
      const { cliStatus, selectedModel } = useSettingsStore.getState();
      const current = providerOf(selectedModel);
      if (cliStatus[current]?.found === false) {
        setIssue({ provider: current, kind: "missing", message: "" });
      } else if (cliStatus[current]?.loggedIn === false) {
        setIssue({ provider: current, kind: "login", message: "" });
      }
    });
    return () => {
      cancelled = true;
    };
  }, [isInitialized, provider, refreshCliStatus, setIssue]);

  // Re-check when the window regains focus, e.g. after signing in through the browser
  useEffect(() => {
    if (!isInitialized) return;
    const onFocus = () => void refreshCliStatus();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [isInitialized, refreshCliStatus]);

  if (!issue) return null;
  const copy = describeIssue(issue, cliStatus[issue.provider]?.version ?? null);
  const action = copy.action;
  const cli = CLI_NAMES[issue.provider];

  const handleInstall = async () => {
    const installing = issue;
    const name = CLI_NAMES[installing.provider];
    const previousVersion = cliStatus[installing.provider]?.version ?? null;
    setIsInstalling(true);
    setInstallLog([]);
    setInstallError(null);
    try {
      await installCli(installing.provider);
      await refreshCliStatus();
      const status = useSettingsStore.getState().cliStatus[installing.provider];
      const version = status?.version ?? null;
      const wasUpdate = installing.kind === "update" || installing.kind === "outdated";
      if (wasUpdate && version && version === previousVersion) {
        toast.success(`${name} ${version} is already the newest version`, {
          description:
            installing.kind === "outdated"
              ? `So the update can't fix this model. Pick another model in the sidebar.`
              : undefined,
        });
        setIssue(null);
      } else if (wasUpdate && version) {
        toast.success(`${name} updated to ${version}`);
        setIssue(null);
      } else if (status?.loggedIn === false) {
        toast.success(`${CLI_NAMES[installing.provider]} is installed`, {
          description: `Now sign in with your ${ACCOUNT_NAMES[installing.provider]} account.`,
        });
        setIssue({ provider: installing.provider, kind: "login", message: "" });
      } else {
        toast.success(`${CLI_NAMES[installing.provider]} is ready`);
        setIssue(null);
      }
    } catch (error) {
      setInstallError(String(error));
    } finally {
      setIsInstalling(false);
    }
  };

  const handleOpenTerminal = async (type: CliAction) => {
    setIsOpening(true);
    try {
      await openCliTerminal(issue.provider, type);
      toast.info("Follow the steps in the window that opened, then click Check again");
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsOpening(false);
    }
  };

  const handleCheckAgain = async () => {
    setIsChecking(true);
    try {
      await refreshCliStatus();
      const status = useSettingsStore.getState().cliStatus[issue.provider];
      if (!status?.found) {
        toast.error(`${cli} still isn't installed`);
      } else if (status.loggedIn === false) {
        setIssue({ provider: issue.provider, kind: "login", message: "" });
        toast.error(`${cli} is installed but not signed in yet`);
      } else {
        toast.success(`${cli} is ready`);
        setIssue(null);
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsChecking(false);
    }
  };

  const installs = action?.type === "install" || action?.type === "update";
  const output = !installs
    ? issue.message
    : (installError ?? (installLog.length > 0 ? installLog.join("\n") : issue.message));

  return (
    <Dialog open onOpenChange={(open) => !open && setIssue(null)}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{copy.title}</DialogTitle>
          <DialogDescription>
            {isInstalling
              ? `Downloading and setting up ${cli}. This can take a few minutes; you can close this window and keep working.`
              : installError
                ? `${cli} couldn't be set up automatically. Check your internet connection and try again, or run the installer in a window to see what's going wrong.`
                : copy.description}
          </DialogDescription>
        </DialogHeader>
        {output && (
          <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-words rounded bg-muted p-2 text-[11px] text-muted-foreground">
            {output}
          </pre>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => setIssue(null)}>
            Close
          </Button>
          {action && installs && (
            <>
              {installError && (
                <Button
                  variant="secondary"
                  onClick={() => handleOpenTerminal(action.type)}
                  disabled={isOpening}
                >
                  {isOpening && <IconLoader2 className="animate-spin" />}
                  Install in a window
                </Button>
              )}
              <Button onClick={handleInstall} disabled={isInstalling}>
                {isInstalling && <IconLoader2 className="animate-spin" />}
                {isInstalling ? "Installing…" : installError ? "Try again" : action.label}
              </Button>
            </>
          )}
          {action && !installs && (
            <>
              <Button variant="secondary" onClick={handleCheckAgain} disabled={isChecking}>
                {isChecking && <IconLoader2 className="animate-spin" />}
                Check again
              </Button>
              <Button onClick={() => handleOpenTerminal(action.type)} disabled={isOpening}>
                {isOpening && <IconLoader2 className="animate-spin" />}
                {action.label}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
