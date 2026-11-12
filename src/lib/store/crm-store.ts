import { create } from "zustand";
import { toast } from "sonner";
import {
  connectCrm,
  disconnectCrm,
  getCrmConnection,
  pushPeopleToCrm,
  type CrmConnection,
  type PushSummary,
} from "@/lib/tauri/commands";

interface CrmState {
  connection: CrmConnection | null;
  isLoaded: boolean;
  dialogOpen: boolean;
  /** People waiting to be pushed once the connection is made */
  pendingIds: number[] | null;
  isPushing: boolean;
  load: () => Promise<void>;
  openDialog: (pendingIds?: number[]) => void;
  closeDialog: () => void;
  connect: (url: string, key: string) => Promise<void>;
  disconnect: () => Promise<void>;
  /** Resolves true when the push went through */
  push: (personIds: number[]) => Promise<boolean>;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** One line for the toast: what happened to everyone who was sent */
export function describePush(summary: PushSummary): string {
  const parts = [
    summary.created && `${plural(summary.created, "new contact", "new contacts")} added`,
    summary.updated && `${plural(summary.updated, "contact", "contacts")} updated`,
    summary.unchanged && `${plural(summary.unchanged, "was", "were")} already there`,
    summary.archived &&
      `${plural(summary.archived, "is", "are")} archived in the CRM and left alone`,
    summary.invalid && `${plural(summary.invalid, "email wasn't", "emails weren't")} usable`,
    summary.noEmail &&
      `${plural(summary.noEmail, "person has", "people have")} no email yet, so wasn't sent`,
  ].filter(Boolean);
  return parts.length ? parts.join(", ") : "Nothing was sent";
}

export const useCrmStore = create<CrmState>((set, get) => ({
  connection: null,
  isLoaded: false,
  dialogOpen: false,
  pendingIds: null,
  isPushing: false,

  load: async () => {
    try {
      set({ connection: await getCrmConnection(), isLoaded: true });
    } catch (error) {
      console.error("Failed to load CRM connection:", error);
      set({ isLoaded: true });
    }
  },

  openDialog: (pendingIds) => set({ dialogOpen: true, pendingIds: pendingIds ?? null }),
  closeDialog: () => set({ dialogOpen: false, pendingIds: null }),

  connect: async (url, key) => {
    const connection = await connectCrm(url, key);
    const pending = get().pendingIds;
    set({ connection, dialogOpen: false, pendingIds: null });
    toast.success(`Connected to ${connection.url.replace(/^https?:\/\//, "")}`);
    if (pending?.length) await get().push(pending);
  },

  disconnect: async () => {
    await disconnectCrm();
    set({ connection: null });
    toast.success("Disconnected from the CRM");
  },

  push: async (personIds) => {
    if (!get().connection) {
      get().openDialog(personIds);
      return false;
    }
    if (get().isPushing) return false;
    set({ isPushing: true });
    const toastId = toast.loading(
      `Sending ${plural(personIds.length, "person", "people")} to the CRM…`
    );
    try {
      const summary = await pushPeopleToCrm(personIds);
      const sent = summary.created + summary.updated + summary.unchanged + summary.archived;
      const message = describePush(summary);
      if (sent > 0) toast.success(message, { id: toastId });
      else toast.warning(message, { id: toastId });
      return sent > 0;
    } catch (error) {
      toast.error(String(error), { id: toastId, duration: 10_000 });
      return false;
    } finally {
      set({ isPushing: false });
    }
  },
}));
