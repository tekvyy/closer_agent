import { create } from "zustand";
import { toast } from "sonner";
import { downloadAppUpdate } from "@/lib/tauri/commands";

const ATTEMPTS = 3;

type UpdateStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up_to_date" }
  | { kind: "ready"; version: string }
  | { kind: "failed"; message: string };

interface UpdateState {
  status: UpdateStatus;
  /** The "ready to install" dialog */
  dialogOpen: boolean;
  setDialogOpen: (open: boolean) => void;
  /** Checks for and downloads a newer version. `manual` reports the result even when there's nothing new */
  check: (manual: boolean) => Promise<void>;
}

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

export const useUpdateStore = create<UpdateState>()((set, get) => ({
  status: { kind: "idle" },
  dialogOpen: false,
  setDialogOpen: (open) => set({ dialogOpen: open }),
  check: async (manual) => {
    const current = get().status;
    if (current.kind === "checking") return;
    if (current.kind === "ready") {
      set({ dialogOpen: true });
      return;
    }
    set({ status: { kind: "checking" } });
    try {
      const version = await withRetries(downloadAppUpdate);
      if (version) {
        set({ status: { kind: "ready", version }, dialogOpen: true });
      } else {
        set({ status: { kind: "up_to_date" } });
        if (manual) toast.success("Closer is up to date");
      }
    } catch (error) {
      console.error("Update check failed:", error);
      set({ status: { kind: "failed", message: String(error) } });
      if (manual) toast.error("Couldn't check for updates", { description: String(error) });
    }
  },
}));
