import { create } from "zustand";
import { toast } from "sonner";
import {
  checkCli,
  getSettings,
  updateSettings as updateSettingsCmd,
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
/** The one model each account researches with, so nobody has to pick */
export const RESEARCH_MODELS = modelConfig.researchModels as Record<ModelProvider, ModelId>;

export function isModelId(value: string): value is ModelId {
  return MODEL_OPTIONS.some((model) => model.value === value);
}

export function providerOf(model: ModelId): ModelProvider {
  return MODEL_OPTIONS.find((option) => option.value === model)?.provider ?? "claude";
}

interface SettingsState {
  /** The research model of the account in use (see RESEARCH_MODELS) */
  selectedModel: ModelId;
  isLoading: boolean;
  isInitialized: boolean;
  isUpdatingModel: boolean;
  /** Install/sign-in state of each provider's CLI; absent until first checked */
  cliStatus: Partial<Record<ModelProvider, CliStatus>>;
  /** CLI problem shown in the CLI dialog (install, update, sign in); null when closed */
  cliIssue: CliIssue | null;
  setCliIssue: (issue: CliIssue | null) => void;
  loadSettings: () => Promise<void>;
  /** Re-check both CLIs; switches away from a provider whose CLI is missing */
  refreshCliStatus: () => Promise<void>;
  /** Research with this account from now on */
  setProvider: (provider: ModelProvider) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>()((set, get) => ({
  selectedModel: DEFAULT_MODEL,
  isLoading: false,
  isInitialized: false,
  isUpdatingModel: false,
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

    // Jobs would fail on a missing CLI, so move to an account whose CLI is installed
    const { selectedModel, isInitialized, setProvider } = get();
    const currentProvider = providerOf(selectedModel);
    if (!isInitialized || cliStatus[currentProvider]?.found !== false) return;
    const fallback = PROVIDERS.find((provider) => cliStatus[provider]?.found);
    if (!fallback) return;
    await setProvider(fallback);
  },

  setProvider: async (provider: ModelProvider) => {
    const model = RESEARCH_MODELS[provider];
    const previousModel = get().selectedModel;
    if (model === previousModel) return;

    set({ isUpdatingModel: true });
    try {
      // Wait for the backend to confirm before showing the change
      await updateSettingsCmd(model);
      set({ selectedModel: model });
      toast.success(`Research now uses your ${ACCOUNT_NAMES[provider]} account`);
    } catch (error) {
      console.error("Failed to change the AI account:", error);
      set({ selectedModel: previousModel });
      toast.error("Couldn't change the AI account", {
        description: "The choice could not be saved.",
        action: {
          label: "Retry",
          onClick: () => get().setProvider(provider),
        },
      });
    } finally {
      set({ isUpdatingModel: false });
    }
  },
}));
