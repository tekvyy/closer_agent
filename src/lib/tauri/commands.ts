import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  Lead,
  NewLead,
  Person,
  PersonWithCompany,
  NewPerson,
  Prompt,
  PromptType,
  ScoringConfig,
  LeadScore,
  LeadWithScore,
  StreamEvent,
  ResearchResult,
  AdjacentResult,
  OnboardingStatus,
  Job,
  JobLog,
} from "./types";

// ============================================================================
// Lead Commands
// ============================================================================

export async function getLead(id: number): Promise<Lead | null> {
  return invoke("get_lead", { id });
}

export async function getAllLeads(): Promise<Lead[]> {
  return invoke("get_all_leads");
}

export async function getAdjacentLeads(currentId: number): Promise<AdjacentResult> {
  return invoke("get_adjacent_leads", { currentId });
}

export async function insertLead(data: NewLead): Promise<number> {
  return invoke("insert_lead", { data });
}

export async function updateLeadUserStatus(leadId: number, status: string): Promise<void> {
  return invoke("update_lead_user_status", { leadId, status });
}

export async function updateLeadNotes(leadId: number, notes: string): Promise<void> {
  return invoke("update_lead_notes", { leadId, notes });
}

export async function deleteLeads(leadIds: number[]): Promise<number> {
  return invoke("delete_leads", { leadIds });
}

// ============================================================================
// Person Commands
// ============================================================================

export async function getPerson(id: number): Promise<PersonWithCompany | null> {
  return invoke("get_person", { id });
}

export async function getPeopleForLead(leadId: number): Promise<Person[]> {
  return invoke("get_people_for_lead", { leadId });
}

export async function getAllPeople(): Promise<PersonWithCompany[]> {
  return invoke("get_all_people");
}

export async function getAdjacentPeople(currentId: number): Promise<AdjacentResult> {
  return invoke("get_adjacent_people", { currentId });
}

export async function insertPerson(data: NewPerson): Promise<number> {
  return invoke("insert_person", { data });
}

export async function updatePersonUserStatus(personId: number, status: string): Promise<void> {
  return invoke("update_person_user_status", { personId, status });
}

export async function deletePeople(personIds: number[]): Promise<number> {
  return invoke("delete_people", { personIds });
}

// ============================================================================
// Prompt Commands
// ============================================================================

export async function getPromptByType(promptType: PromptType): Promise<Prompt | null> {
  return invoke("get_prompt_by_type", { promptType });
}

export async function savePromptByType(promptType: PromptType, content: string): Promise<number> {
  return invoke("save_prompt_by_type", { promptType, content });
}

// ============================================================================
// Company Overview Commands
// ============================================================================

export async function saveCompanyOverview(content: string): Promise<number> {
  return savePromptByType("company_overview", content);
}

// ============================================================================
// Onboarding Commands
// ============================================================================

export async function getOnboardingStatus(): Promise<OnboardingStatus> {
  return invoke("get_onboarding_status");
}

// ============================================================================
// Scoring Config Commands
// ============================================================================

export async function getActiveScoringConfig(): Promise<ScoringConfig | null> {
  return invoke("get_active_scoring_config");
}

export async function saveScoringConfig(
  name: string,
  requiredCharacteristics: string,
  demandSignifiers: string,
  tierHotMin: number,
  tierWarmMin: number,
  tierNurtureMin: number,
  id?: number
): Promise<number> {
  return invoke("save_scoring_config", {
    name,
    requiredCharacteristics,
    demandSignifiers,
    tierHotMin,
    tierWarmMin,
    tierNurtureMin,
    id,
  });
}

// ============================================================================
// Lead Score Commands
// ============================================================================

export async function getLeadScore(leadId: number): Promise<LeadScore | null> {
  return invoke("get_lead_score", { leadId });
}

export async function getLeadsWithScores(): Promise<LeadWithScore[]> {
  return invoke("get_leads_with_scores");
}

// ============================================================================
// Research Commands
// ============================================================================

export async function startResearch(
  leadId: number,
  onEvent: (event: StreamEvent) => void,
  customPrompt?: string
): Promise<ResearchResult> {
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;

  return invoke("start_research", {
    leadId,
    customPrompt,
    onEvent: channel,
  });
}

export async function startPersonResearch(
  personId: number,
  onEvent: (event: StreamEvent) => void,
  customPrompt?: string
): Promise<ResearchResult> {
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;

  return invoke("start_person_research", {
    personId,
    customPrompt,
    onEvent: channel,
  });
}

export async function startFindLeads(
  icpDescription: string,
  onEvent: (event: StreamEvent) => void
): Promise<ResearchResult> {
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;

  return invoke("start_find_leads", {
    icpDescription,
    onEvent: channel,
  });
}

export async function killJob(jobId: string): Promise<void> {
  return invoke("kill_job", { jobId });
}

// ============================================================================
// Scoring Commands
// ============================================================================

export async function startScoring(
  leadId: number,
  onEvent: (event: StreamEvent) => void
): Promise<ResearchResult> {
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;

  return invoke("start_scoring", {
    leadId,
    onEvent: channel,
  });
}

// ============================================================================
// Conversation Generation Commands
// ============================================================================

export async function startConversationGeneration(
  personId: number,
  onEvent: (event: StreamEvent) => void
): Promise<ResearchResult> {
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;

  return invoke("start_conversation_generation", {
    personId,
    onEvent: channel,
  });
}

// ============================================================================
// Job Commands
// ============================================================================

export async function getJobsActive(): Promise<Job[]> {
  return invoke("get_jobs_active");
}

export async function getJobsRecent(limit?: number): Promise<Job[]> {
  return invoke("get_jobs_recent", { limit });
}

export async function getJobById(jobId: string): Promise<Job | null> {
  return invoke("get_job_by_id", { jobId });
}

export async function getJobLogs(
  jobId: string,
  afterSequence?: number,
  limit?: number
): Promise<JobLog[]> {
  return invoke("get_job_logs_cmd", { jobId, afterSequence, limit });
}

export async function deleteJob(jobId: string): Promise<void> {
  return invoke("delete_job_cmd", { jobId });
}

// ============================================================================
// Settings Commands
// ============================================================================

export interface Settings {
  model: string;
  useChrome: boolean;
  /** Start the next research step automatically when one finishes */
  autoResearch: boolean;
  updatedAt: number;
}

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function updateSettings(model: string, useChrome: boolean): Promise<void> {
  return invoke("update_settings", { model, useChrome });
}

export async function setAutoResearch(enabled: boolean): Promise<void> {
  return invoke("set_auto_research", { enabled });
}

// ============================================================================
// Agent CLI commands
// ============================================================================

/** Agent CLI that runs jobs: Claude Code, or Codex signed in with ChatGPT */
export type CliProvider = "claude" | "codex";

/**
 * Why the CLI dialog is open. "outdated" comes from a job whose model the CLI rejected;
 * "update" is the user asking to update from the model menu.
 */
export type CliIssueKind = "missing" | "outdated" | "update" | "login" | "limit";

export type CliAction = "install" | "update" | "login";

/** Payload of the `cli-issue` event emitted when a job fails for a fixable CLI reason */
export interface CliIssue {
  provider: CliProvider;
  kind: CliIssueKind;
  message: string;
}

export interface CliStatus {
  found: boolean;
  path: string | null;
  /** null when the sign-in state could not be determined */
  loggedIn: boolean | null;
  /** Installed version, e.g. "2.1.307"; null when it couldn't be read */
  version: string | null;
}

export async function checkCli(provider: CliProvider): Promise<CliStatus> {
  return invoke("check_cli", { provider });
}

/** Opens a terminal window running the install, update or sign-in command */
export async function openCliTerminal(provider: CliProvider, action: CliAction): Promise<void> {
  return invoke("open_cli_terminal", { provider, action });
}

/** Payload of the `cli-install-output` event: one line of installer output */
export interface CliInstallOutput {
  provider: CliProvider;
  line: string;
}

/**
 * Installs or updates the CLI in the background with its official installer (no
 * Node.js needed). Progress arrives as `cli-install-output` events; resolves with the
 * installed CLI's path.
 */
export async function installCli(provider: CliProvider): Promise<string> {
  return invoke("install_cli", { provider });
}

// ============================================================================
// App update commands
// ============================================================================

/**
 * Checks for a newer app version and downloads it. Resolves with the version ready to
 * install, or null when the app is up to date.
 */
export async function downloadAppUpdate(): Promise<string | null> {
  return invoke("download_app_update");
}

/** Installs the downloaded update and restarts the app into it */
export async function installAppUpdate(): Promise<void> {
  return invoke("install_app_update");
}
