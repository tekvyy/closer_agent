# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Development (starts Vite + Tauri with hot reload)
bun run tauri:dev

# Build production app
bun run tauri:build

# Frontend only (Vite dev server on localhost:3000)
bun run dev

# Lint and format
bun run lint
bun run lint:fix
bun run format
bun run format:check

# Type check
tsc -b
```

## Architecture Overview

Closer is a **Tauri 2 desktop application** for B2B lead research and qualification. It uses Claude CLI as an AI backend for company/person research, scoring, and conversation generation.

### Tech Stack

- **Frontend**: React 19 + TypeScript + Vite + Tailwind CSS 4
- **Backend**: Rust + Tauri 2 + SQLite (rusqlite with WAL mode)
- **State**: Zustand with Immer middleware
- **UI**: Radix UI primitives + Tabler Icons + Sonner toasts

### Directory Structure

```
src/                          # React frontend
├── pages/                    # Page components (list.tsx, detail.tsx)
├── components/
│   ├── ui/                   # Radix-wrapped primitives (Button, Dialog, etc.)
│   ├── leads/, people/       # Feature-specific components
│   └── stream-panel/         # Real-time job output display
└── lib/
    ├── store/                # Zustand stores (leads-store.ts, people-store.ts)
    ├── tauri/                # Backend integration (commands.ts, event-bridge.ts)
    └── hooks/                # Data fetching hooks (use-leads.ts, use-people.ts)

src-tauri/src/               # Rust backend
├── commands/                # Tauri command handlers
│   ├── database.rs          # Lead/Person/Score CRUD
│   ├── research.rs          # Job management (research, scoring)
│   └── prompts.rs           # Prompt storage
├── db/                      # SQLite schema and queries
├── jobs/                    # Async job queue (5 concurrent, 10min timeout)
└── events.rs                # Event emission to frontend
```

### Data Flow

1. **Frontend → Backend**: Tauri `invoke()` calls defined in `src/lib/tauri/commands.ts`
2. **Backend → Frontend**:
   - Streaming: `Channel<StreamEvent>` for real-time job output
   - Reactive updates: Tauri events (`lead-updated`, `person-updated`, etc.) handled by `event-bridge.ts`
3. **State Management**: Zustand stores with Map-based normalization for O(1) lookups

### Job System

Research/scoring jobs spawn Claude CLI subprocesses:

- Max 5 concurrent jobs (semaphore-based)
- 10-minute timeout per job
- Output streams to frontend via Tauri channels
- Results parsed and stored in SQLite on completion
- Jobs past the concurrency limit wait in the queue for a free slot
- Autopilot (`src-tauri/src/autopilot.rs`, `auto_research` setting): a finished job queues the next step (find leads → company research → scoring + people research → talking points)

### Key Patterns

- **Normalized state**: `Map<id, Entity>` in stores for efficient updates
- **Event-driven updates**: Backend emits events → frontend invalidates cached data
- **Streaming UI**: `StreamPanelStore` persists job logs across navigation
- **Selection context**: Centralized selection store with keyboard shortcuts (Cmd+A, Escape)

## Database

Location: `~/.local/share/qual/data.db`

Tables: `leads`, `people`, `prompts`, `scoring_config`, `lead_scores`

## Important Files

| Purpose        | Path                            |
| -------------- | ------------------------------- |
| App routing    | `src/App.tsx`                   |
| Lead store     | `src/lib/store/leads-store.ts`  |
| Tauri commands | `src/lib/tauri/commands.ts`     |
| Event bridge   | `src/lib/tauri/event-bridge.ts` |
| Backend setup  | `src-tauri/src/lib.rs`          |
| Job queue      | `src-tauri/src/jobs/queue.rs`   |
| DB schema      | `src-tauri/src/db/schema.rs`    |

## Gina's CRM (where "Push to CRM" goes)

This fork (tekvyy/closer_agent) is Gina J. Waota's research tool, which she calls "GynaJ research". It
pushes the people she selects into her CRM at https://ginaj.me.

- **CRM code on this laptop:** `/Users/singh/Desktop/Gyna/crm` (repo tekvyy/gina_crm, a Next.js app
  that Railway auto-deploys from `main`). Its `README.md` has a "Research tool (Closer)" section.
- **The CRM side of a push:** `src/app/api/research/people/route.ts` (key check, limits, up to 200 people
  per push) and `src/core/research-push.ts` (creates contacts with source `research`, fills blanks on
  existing ones, leaves archived ones alone, returns created/updated/unchanged/archived/invalid per person).
- **This side:** `src-tauri/src/crm.rs` (saves the address and `gk_…` key, `normalise_url` requires
  https except for localhost, sends the push), `src/components/crm/crm-connection.tsx`,
  `src/lib/store/crm-store.ts`, `src/lib/tauri/commands.ts`, and the push button in
  `src/components/people/people-list-with-selection.tsx`.
- **Debugging a push:** a bug can be on either side, so check both. Test against a local CRM
  before touching production: `npx next dev -p 3100` in the CRM (port 3000 is taken by this app's
  Vite server), then connect Closer to `http://localhost:3100` with a key made at `/admin/research-tool`.
- **Releases:** pushing a `v*` tag builds a release and auto-updates every installed copy, Gina's included.
  Tags need a message (`git tag -a`, since `tag.gpgsign` is on). Use `gh … -R tekvyy/closer_agent`, because
  `gh` defaults to the upstream chaitanyya/sales.
