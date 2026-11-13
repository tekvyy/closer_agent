"use client";

import * as React from "react";
import { SelectionProvider } from "./selection-provider";
import { FloatingActionBar } from "./floating-action-bar";
import { CollapsibleStatusGroup } from "@/components/ui/collapsible-status-group";
import { Checkbox } from "@/components/ui/checkbox";
import { useSelectionStore } from "@/lib/store/selection-store";
import type { ActionConfig } from "./action-command-menu";
import type { SelectionEntityType } from "@/lib/store/selection-store";

type StatusConfigType = "lead_user" | "person_user";

interface SelectableEntityListProps<T, S extends string> {
  /** Type of entity for selection tracking */
  entityType: SelectionEntityType;
  /** Grouped items by status */
  groupedItems: Record<S, T[]>;
  /** Ordered list of status values to display */
  statusOrder: readonly S[];
  /** Config type for status badge styling */
  configType: StatusConfigType;
  /** Get the unique ID for an item */
  getItemId: (item: T) => number;
  /** Render function for each row */
  renderRow: (item: T) => React.ReactNode;
  /** Actions available for selected items */
  actions: ActionConfig[];
}

export function SelectableEntityList<T, S extends string>({
  entityType,
  groupedItems,
  statusOrder,
  configType,
  getItemId,
  renderRow,
  actions,
}: SelectableEntityListProps<T, S>) {
  // Collect all item IDs in display order
  const allIds = React.useMemo(() => {
    const ids: number[] = [];
    statusOrder.forEach((status) => {
      const items = groupedItems[status];
      if (items) {
        items.forEach((item) => ids.push(getItemId(item)));
      }
    });
    return ids;
  }, [groupedItems, statusOrder, getItemId]);

  return (
    <SelectionProvider entityType={entityType} allIds={allIds}>
      <div className="flex-1 overflow-auto">
        {allIds.length > 0 && <SelectAllBar ids={allIds} />}
        {statusOrder.map((status) => {
          const items = groupedItems[status];
          if (!items || items.length === 0) return null;

          return (
            <CollapsibleStatusGroup
              key={status}
              status={status}
              count={items.length}
              configType={configType}
              action={<GroupCheckbox ids={items.map(getItemId)} />}
            >
              {items.map((item) => (
                <Row key={getItemId(item)} item={item} render={renderRow} />
              ))}
            </CollapsibleStatusGroup>
          );
        })}
      </div>
      <FloatingActionBar actions={actions} />
    </SelectionProvider>
  );
}

/** How many of these ids are selected, as a primitive so rows don't re-render on every change */
function useSelectedAmong(ids: number[]) {
  return useSelectionStore((state) => {
    let count = 0;
    for (const id of ids) if (state.selectedIds.has(id)) count++;
    return count;
  });
}

function checkState(selected: number, total: number): boolean | "indeterminate" {
  if (selected === 0) return false;
  return selected === total ? true : "indeterminate";
}

/** Adds every id when any are unselected, otherwise removes them all */
function useToggleMany(ids: number[], allSelected: boolean) {
  return React.useCallback(() => {
    const { selectedIds, selectAll } = useSelectionStore.getState();
    const next = new Set(selectedIds);
    for (const id of ids) {
      if (allSelected) next.delete(id);
      else next.add(id);
    }
    selectAll(Array.from(next));
  }, [ids, allSelected]);
}

function SelectAllBar({ ids }: { ids: number[] }) {
  const selected = useSelectedAmong(ids);
  const allSelected = selected === ids.length;
  const toggle = useToggleMany(ids, allSelected);

  return (
    <label className="flex items-center gap-2 px-3 py-2 text-sm border-b border-white/5 cursor-pointer select-none hover:bg-white/[0.03]">
      <div className="w-4 shrink-0 flex items-center justify-center">
        <Checkbox checked={checkState(selected, ids.length)} onCheckedChange={toggle} />
      </div>
      <span className="text-muted-foreground">
        {allSelected ? "Deselect all" : `Select all ${ids.length}`}
      </span>
      {selected > 0 && !allSelected && (
        <span className="text-xs text-muted-foreground/70">{selected} selected</span>
      )}
    </label>
  );
}

function GroupCheckbox({ ids }: { ids: number[] }) {
  const selected = useSelectedAmong(ids);
  const allSelected = selected === ids.length;
  const toggle = useToggleMany(ids, allSelected);

  return (
    <Checkbox
      checked={checkState(selected, ids.length)}
      onCheckedChange={toggle}
      aria-label={allSelected ? "Deselect this group" : "Select this group"}
      title={allSelected ? "Deselect this group" : "Select everyone in this group"}
    />
  );
}

function Row<T>({ item, render }: { item: T; render: (item: T) => React.ReactNode }) {
  return <>{render(item)}</>;
}
