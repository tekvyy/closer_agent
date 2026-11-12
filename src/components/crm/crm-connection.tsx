"use client";

import { useEffect, useState } from "react";
import { IconLoader2, IconPlugConnected } from "@tabler/icons-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useCrmStore } from "@/lib/store/crm-store";

/** Sidebar row: shows whether the CRM is connected, and opens the connection dialog */
export function CrmConnectionRow() {
  const connection = useCrmStore((state) => state.connection);
  const isLoaded = useCrmStore((state) => state.isLoaded);
  const load = useCrmStore((state) => state.load);
  const openDialog = useCrmStore((state) => state.openDialog);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <>
      <button
        onClick={() => openDialog()}
        disabled={!isLoaded}
        title={
          connection
            ? `Connected to ${connection.url}`
            : "Connect to your CRM to push people into it"
        }
        className="flex items-center gap-2 w-full px-2 py-1 rounded text-muted-foreground hover:bg-white/5 hover:text-foreground transition-colors text-sm outline-none disabled:opacity-50"
      >
        <IconPlugConnected className="size-3.5" />
        <span className="flex-1 text-left">CRM</span>
        <span className={`text-xs ${connection ? "text-green-500" : ""}`}>
          {connection ? "Connected" : "Connect"}
        </span>
      </button>
      <CrmConnectionDialog />
    </>
  );
}

function CrmConnectionDialog() {
  const open = useCrmStore((state) => state.dialogOpen);
  const closeDialog = useCrmStore((state) => state.closeDialog);
  return (
    <Dialog open={open} onOpenChange={(next) => (next ? undefined : closeDialog())}>
      <DialogContent>
        {/* Mounted only while open, so every opening starts from the saved connection */}
        <ConnectionForm />
      </DialogContent>
    </Dialog>
  );
}

function ConnectionForm() {
  const connection = useCrmStore((state) => state.connection);
  const pendingIds = useCrmStore((state) => state.pendingIds);
  const closeDialog = useCrmStore((state) => state.closeDialog);
  const connect = useCrmStore((state) => state.connect);
  const disconnect = useCrmStore((state) => state.disconnect);

  const [url, setUrl] = useState(connection?.url ?? "");
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!url.trim() || !key.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await connect(url, key);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <DialogHeader>
        <DialogTitle>{connection ? "CRM connection" : "Connect to your CRM"}</DialogTitle>
        <DialogDescription>
          {connection
            ? `Connected to ${connection.url} with the key "${connection.keyName}". Paste a new key below to replace it.`
            : "In your CRM, open Settings, then Research tool, and make a key for this computer. Paste the address and key here."}
        </DialogDescription>
      </DialogHeader>
      {pendingIds?.length ? (
        <p className="text-sm text-muted-foreground">
          Once connected, the {pendingIds.length === 1 ? "person" : `${pendingIds.length} people`}{" "}
          you picked will be sent.
        </p>
      ) : null}
      <form onSubmit={handleSubmit} className="grid gap-4">
        <div className="grid gap-2">
          <Label htmlFor="crm-url">CRM address</Label>
          <Input
            id="crm-url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://ginaj.me"
            autoComplete="off"
            spellCheck={false}
            required
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="crm-key">Connection key</Label>
          <Input
            id="crm-key"
            type="password"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder="gk_…"
            autoComplete="off"
            spellCheck={false}
            required
          />
        </div>
        {error ? <p className="text-sm text-destructive">{error}</p> : null}
        <DialogFooter className="gap-2 sm:justify-between">
          {connection ? (
            <Button
              type="button"
              variant="ghost"
              onClick={async () => {
                await disconnect();
                closeDialog();
              }}
            >
              Disconnect
            </Button>
          ) : (
            <span />
          )}
          <Button type="submit" disabled={busy || !url.trim() || !key.trim()}>
            {busy ? <IconLoader2 className="size-3.5 animate-spin" /> : null}
            {busy ? "Checking…" : "Connect"}
          </Button>
        </DialogFooter>
      </form>
    </>
  );
}
