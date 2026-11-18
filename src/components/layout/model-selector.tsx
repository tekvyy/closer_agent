"use client";

import { Fragment, useEffect } from "react";
import {
  IconSparkles,
  IconChevronDown,
  IconLoader2,
  IconDownload,
  IconRefresh,
  IconLogin,
} from "@tabler/icons-react";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuLabel,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import type { CliIssueKind } from "@/lib/tauri/commands";
import {
  useSettingsStore,
  providerOf,
  PROVIDERS,
  CLI_NAMES,
  ACCOUNT_NAMES,
  ModelProvider,
} from "@/lib/store/settings-store";

/** Install, update and sign-in all run through the CLI dialog, which shows progress */
function openCliDialog(provider: ModelProvider, kind: CliIssueKind) {
  useSettingsStore.getState().setCliIssue({ provider, kind, message: "" });
}

/**
 * Which AI account research runs on. Closer picks the model itself, so the only choice
 * is between a Claude and a ChatGPT account, for people who have both.
 */
export function AccountSelector() {
  const provider = useSettingsStore((state) => providerOf(state.selectedModel));
  const setProvider = useSettingsStore((state) => state.setProvider);
  const loadSettings = useSettingsStore((state) => state.loadSettings);
  const isInitialized = useSettingsStore((state) => state.isInitialized);
  const isUpdatingModel = useSettingsStore((state) => state.isUpdatingModel);
  const cliStatus = useSettingsStore((state) => state.cliStatus);

  // Hide an account only once its CLI is confirmed missing
  const installedProviders = PROVIDERS.filter((p) => cliStatus[p]?.found !== false);
  const missingProviders = PROVIDERS.filter((p) => cliStatus[p]?.found === false);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          title="The AI account Closer researches with"
          className="flex items-center gap-2 w-full px-2 py-1 rounded text-muted-foreground hover:bg-white/5 hover:text-foreground transition-colors text-sm outline-none disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          disabled={!isInitialized || isUpdatingModel}
        >
          {isUpdatingModel ? (
            <IconLoader2 className="size-3.5 animate-spin" />
          ) : (
            <IconSparkles className="size-3.5" />
          )}
          <span className="flex-1 text-left">{ACCOUNT_NAMES[provider]} account</span>
          <IconChevronDown className="size-3" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuLabel>Research with</DropdownMenuLabel>
        <DropdownMenuRadioGroup
          value={provider}
          onValueChange={(value) => !isUpdatingModel && setProvider(value as ModelProvider)}
        >
          {installedProviders.map((p) => (
            <DropdownMenuRadioItem key={p} value={p} disabled={isUpdatingModel}>
              <span>{ACCOUNT_NAMES[p]} account</span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        {missingProviders.map((p) => (
          <DropdownMenuItem key={p} onSelect={() => openCliDialog(p, "missing")}>
            <IconDownload className="size-4" />
            <span>Set up a {ACCOUNT_NAMES[p]} account</span>
          </DropdownMenuItem>
        ))}
        {installedProviders.map((p) => (
          <Fragment key={p}>
            {cliStatus[p]?.loggedIn === false ? (
              <DropdownMenuItem onSelect={() => openCliDialog(p, "login")}>
                <IconLogin className="size-4" />
                <span>Sign in to {ACCOUNT_NAMES[p]}</span>
              </DropdownMenuItem>
            ) : (
              <DropdownMenuItem onSelect={() => openCliDialog(p, "update")}>
                <IconRefresh className="size-4" />
                <span>Update {CLI_NAMES[p]}</span>
              </DropdownMenuItem>
            )}
          </Fragment>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
