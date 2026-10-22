import { create } from "zustand";
import { toast } from "sonner";
import {
  checkCli,
  getSettings,
  updateSettings as updateSettingsCmd,
  setAutoResearch as setAutoResearchCmd,
  type CliIssue,
  type CliProvider,
  type CliStatus,
} from "@/lib/tauri/commands";
import modelConfig from "@/config/claude-models.json";

export type ModelId =
  | "claude-fable-5-1"
  | "claude-opus-5-5"
  | "claude-sonnet-5-5"
  | "claude-haiku-5-5"
  | "gpt-6-astra"
  | "gpt-6.1-sol"
  | "gpt-6-luna";

/** Agent CLI that runs the model: Claude Code, or Codex signed in with ChatGPT */
export type ModelProvider = CliProvider;

export const PROVIDER_LABELS: Record<ModelProvider, string> = {
  claude: "Claude",
  codex: "ChatGPT (Codex)",
};

export const PROVIDERS = Object.keys(PROVIDER_LABELS) as ModelProvider[];

/** Name of the CLI each provider runs through */
export const CLI_NAMES: Record<ModelProvider, string> = {
  claude: "Claude Code",
  codex: "Codex",
};

/** Account the CLI signs in with */
export const ACCOUNT_NAMES: Record<ModelProvider, string> = {
  claude: "Claude",
  codex: "ChatGPT",
};

export interface ModelOption {
  value: ModelId;
  label: string;
  provider: ModelProvider;
  icon: "brain" | "lightning";
  description: string;
}

export const MODEL_OPTIONS = modelConfig.models as ModelOption[];
export const DEFAULT_MODEL = modelConfig.defaultModel as ModelId;

export function isModelId(value: string): value is ModelId {
  return MODEL_OPTIONS.some((model) => model.value === value);
}

export function providerOf(model: ModelId): ModelProvider {
  return MODEL_OPTIONS.find((option) => option.value === model)?.provider ?? "claude";
}

interface SettingsState {
  selectedModel: ModelId;
  useChrome: boolean;
  /** Autopilot: chain research, scoring, people and talking points */
  autoResearch: boolean;
  isLoading: boolean;
  isInitialized: boolean;
  isUpdatingModel: boolean;
  isUpdatingChrome: boolean;
  /** Install/sign-in state of each provider's CLI; absent until first checked */
  cliStatus: Partial<Record<ModelProvider, CliStatus>>;
  /** CLI problem shown in the CLI dialog (install, update, sign in); null when closed */
  cliIssue: CliIssue | null;
  setCliIssue: (issue: CliIssue | null) => void;
  loadSettings: () => Promise<void>;
  /** Re-check both CLIs; switches away from a provider whose CLI is missing */
  refreshCliStatus: () => Promise<void>;
  setSelectedModel: (model: ModelId) => Promise<void>;
  setUseChrome: (useChrome: boolean) => Promise<void>;
  setAutoResearch: (enabled: boolean) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>()((set, get) => ({
  selectedModel: DEFAULT_MODEL,
  useChrome: false,
  autoResearch: true,
  isLoading: false,
  isInitialized: false,
  isUpdatingModel: false,
  isUpdatingChrome: false,
  cliStatus: {},
  cliIssue: null,

  setCliIssue: (issue) => set({ cliIssue: issue }),

  loadSettings: async () => {
    if (get().isInitialized) return;

    set({ isLoading: true });
    try {
      const settings = await getSettings();
      set({
        selectedModel: isModelId(settings.model) ? settings.model : DEFAULT_MODEL,
        useChrome: settings.useChrome,
        autoResearch: settings.autoResearch,
        isInitialized: true,
      });
    } catch (error) {
      console.error("Failed to load settings:", error);
      toast.error("Failed to load settings");
    } finally {
      set({ isLoading: false });
    }
  },

  refreshCliStatus: async () => {
    const entries = await Promise.all(
      PROVIDERS.map(async (provider) => {
        try {
          return [provider, await checkCli(provider)] as const;
        } catch (error) {
          console.error(`Failed to check ${provider} CLI:`, error);
          return [provider, undefined] as const;
        }
      })
    );
    const cliStatus = Object.fromEntries(
      entries.filter(([, status]) => status !== undefined)
    ) as Partial<Record<ModelProvider, CliStatus>>;
    set({ cliStatus });

    // Jobs would fail on a missing CLI, so move to a provider that is installed
    const { selectedModel, isInitialized, setSelectedModel } = get();
    const currentProvider = providerOf(selectedModel);
    if (!isInitialized || cliStatus[currentProvider]?.found !== false) return;
    const fallback = PROVIDERS.find((provider) => cliStatus[provider]?.found);
    if (!fallback) return;

    const fallbackModel =
      providerOf(DEFAULT_MODEL) === fallback
        ? DEFAULT_MODEL
        : MODEL_OPTIONS.find((option) => option.provider === fallback)!.value;
    toast.info(`${CLI_NAMES[currentProvider]} CLI isn't installed`, {
      description: `Switched to ${PROVIDER_LABELS[fallback]} models. Install ${CLI_NAMES[currentProvider]} from the model menu to use ${ACCOUNT_NAMES[currentProvider]} models.`,
    });
    await setSelectedModel(fallbackModel);
  },

  setSelectedModel: async (model: ModelId) => {
    const { useChrome, selectedModel: previousModel } = get();

    // Pessimistic update: set loading state first
    set({ isUpdatingModel: true });

    try {
      // Wait for backend to confirm the update
      await updateSettingsCmd(model, useChrome);

      // Only update UI state after successful backend update
      set({ selectedModel: model });

      // Show success feedback
      const modelLabel = MODEL_OPTIONS.find((m) => m.value === model)?.label || model;
      toast.success(`Model updated to ${modelLabel}`);
    } catch (error) {
      console.error("Failed to update model setting:", error);

      // Revert to previous model on error
      set({ selectedModel: previousModel });

      // Show error with retry option
      toast.error("Failed to update model", {
        description: "The model selection could not be saved.",
        action: {
          label: "Retry",
          onClick: () => get().setSelectedModel(model),
        },
      });
    } finally {
      set({ isUpdatingModel: false });
    }
  },

  setUseChrome: async (useChrome: boolean) => {
    const { selectedModel, useChrome: previousUseChrome } = get();

    // Pessimistic update: set loading state first
    set({ isUpdatingChrome: true });

    try {
      // Wait for backend to confirm the update
      await updateSettingsCmd(selectedModel, useChrome);

      // Only update UI state after successful backend update
      set({ useChrome });

      // Show success feedback
      toast.success(`Browser integration ${useChrome ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to update useChrome setting:", error);

      // Revert to previous value on error
      set({ useChrome: previousUseChrome });

      // Show error with retry option
      toast.error("Failed to update browser integration", {
        description: "The setting could not be saved.",
        action: {
          label: "Retry",
          onClick: () => get().setUseChrome(useChrome),
        },
      });
    } finally {
      set({ isUpdatingChrome: false });
    }
  },

  setAutoResearch: async (enabled: boolean) => {
    const previous = get().autoResearch;
    set({ autoResearch: enabled });
    try {
      await setAutoResearchCmd(enabled);
      toast.success(enabled ? "Autopilot on" : "Autopilot off", {
        description: enabled
          ? "Finished research now starts scoring, people research and talking points."
          : "Each research step now waits for you to start it.",
      });
    } catch (error) {
      console.error("Failed to update autopilot setting:", error);
      set({ autoResearch: previous });
      toast.error("Couldn't change autopilot", { description: String(error) });
    }
  },
}));
