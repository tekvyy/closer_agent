"use client";

import { Fragment, useEffect } from "react";
import {
  IconBrain,
  IconBolt,
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
  MODEL_OPTIONS,
  DEFAULT_MODEL,
  PROVIDER_LABELS,
  PROVIDERS,
  CLI_NAMES,
  ACCOUNT_NAMES,
  ModelId,
  ModelProvider,
} from "@/lib/store/settings-store";

/** Install, update and sign-in all run through the CLI dialog, which shows progress */
function openCliDialog(provider: ModelProvider, kind: CliIssueKind) {
  useSettingsStore.getState().setCliIssue({ provider, kind, message: "" });
}

const ModelIcon = ({ model, className }: { model: ModelId; className?: string }) => {
  if (MODEL_OPTIONS.find((option) => option.value === model)?.icon === "brain") {
    return <IconBrain className={className} />;
  }
  return <IconBolt className={className} />;
};

export function ModelSelector() {
  const selectedModel = useSettingsStore((state) => state.selectedModel);
  const setSelectedModel = useSettingsStore((state) => state.setSelectedModel);
  const loadSettings = useSettingsStore((state) => state.loadSettings);
  const isInitialized = useSettingsStore((state) => state.isInitialized);
  const isUpdatingModel = useSettingsStore((state) => state.isUpdatingModel);
  const cliStatus = useSettingsStore((state) => state.cliStatus);

  // Hide a provider only once its CLI is confirmed missing
  const installedProviders = PROVIDERS.filter((provider) => cliStatus[provider]?.found !== false);
  const missingProviders = PROVIDERS.filter((provider) => cliStatus[provider]?.found === false);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const currentModel =
    MODEL_OPTIONS.find((model) => model.value === selectedModel) ||
    MODEL_OPTIONS.find((model) => model.value === DEFAULT_MODEL)!;

  if (!isInitialized) {
    return (
      <div className="flex items-center gap-2 w-full px-2 py-1 text-muted-foreground text-sm">
        <IconBolt className="size-3.5" />
        <span>{MODEL_OPTIONS.find((model) => model.value === DEFAULT_MODEL)?.label}</span>
      </div>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          className="flex items-center gap-2 w-full px-2 py-1 rounded text-muted-foreground hover:bg-white/5 hover:text-foreground transition-colors text-sm outline-none disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          disabled={isUpdatingModel}
        >
          {isUpdatingModel ? (
            <IconLoader2 className="size-3.5 animate-spin" />
          ) : (
            <ModelIcon model={currentModel.value} className="size-3.5" />
          )}
          <span className="flex-1 text-left">{currentModel.label}</span>
          <IconChevronDown className="size-3" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuRadioGroup
          value={selectedModel}
          onValueChange={(value) => !isUpdatingModel && setSelectedModel(value as ModelId)}
        >
          {installedProviders.map((provider, index) => (
            <Fragment key={provider}>
              {index > 0 && <DropdownMenuSeparator />}
              <DropdownMenuLabel>{PROVIDER_LABELS[provider]}</DropdownMenuLabel>
              {MODEL_OPTIONS.filter((option) => option.provider === provider).map((option) => (
                <DropdownMenuRadioItem
                  key={option.value}
                  value={option.value}
                  disabled={isUpdatingModel}
                >
                  <ModelIcon model={option.value} className="size-4" />
                  <span title={option.description}>{option.label}</span>
                </DropdownMenuRadioItem>
              ))}
            </Fragment>
          ))}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>CLI tools</DropdownMenuLabel>
        {missingProviders.map((provider) => (
          <DropdownMenuItem key={provider} onSelect={() => openCliDialog(provider, "missing")}>
            <IconDownload className="size-4" />
            <span>
              Install {CLI_NAMES[provider]} to use {ACCOUNT_NAMES[provider]} models
            </span>
          </DropdownMenuItem>
        ))}
        {installedProviders.map((provider) =>
          cliStatus[provider]?.loggedIn === false ? (
            <DropdownMenuItem key={provider} onSelect={() => openCliDialog(provider, "login")}>
              <IconLogin className="size-4" />
              <span>
                Sign in to {CLI_NAMES[provider]} ({ACCOUNT_NAMES[provider]} account)
              </span>
            </DropdownMenuItem>
          ) : (
            <DropdownMenuItem key={provider} onSelect={() => openCliDialog(provider, "outdated")}>
              <IconRefresh className="size-4" />
              <span>Update {CLI_NAMES[provider]}</span>
            </DropdownMenuItem>
          )
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
