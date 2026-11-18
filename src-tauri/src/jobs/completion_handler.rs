//! Completion handler for processing job results atomically
//!
//! This module handles the completion of jobs with proper error handling
//! and atomic operations. It processes job results in phases:
//! 1. Verify output files exist and are readable
//! 2. Parse and validate content
//! 3. Update database records (in a transaction)
//! 4. Cleanup output files only after DB is confirmed
//! 5. Record completion state for recovery

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

use super::enrichment::{LeadEnrichment, PersonEnrichment};
use super::found_people::{finalize_people, FoundPerson};
use super::result_parser::{JobMetadata, JobType};
use super::stream_processor::CompletionContext;
use crate::db;
use crate::events;

/// Completion phases for recovery tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionPhase {
    Started,
    FilesVerified,
    ContentParsed,
    DatabaseUpdated,
    FilesCleanedUp,
    Completed,
    Failed,
}

/// Error types for completion handling
#[derive(Debug)]
pub enum CompletionError {
    FileNotFound(PathBuf),
    FileReadError(PathBuf, std::io::Error),
    ParseError(String),
    DatabaseError(String),
    ValidationError(String),
}

impl std::fmt::Display for CompletionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompletionError::FileNotFound(path) => write!(f, "Output file not found: {:?}", path),
            CompletionError::FileReadError(path, e) => {
                write!(f, "Failed to read {:?}: {}", path, e)
            }
            CompletionError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            CompletionError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            CompletionError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

/// Verified output files
pub struct VerifiedOutputs {
    pub primary_content: String,
    pub secondary_content: Option<String>,
    pub enrichment_content: Option<String>,
}

/// Parsed output content based on job type
pub enum ParsedOutput {
    CompanyResearch {
        profile: String,
        people: Option<Vec<serde_json::Value>>,
        enrichment: Option<LeadEnrichment>,
    },
    PersonResearch {
        profile: String,
        enrichment: Option<PersonEnrichment>,
    },
    Scoring {
        score_data: serde_json::Value,
    },
    Conversation {
        topics: String,
    },
    LeadFinder {
        leads: Vec<serde_json::Value>,
    },
}

/// Handles job completion with atomic operations
pub struct CompletionHandler {
    db_conn: Arc<std::sync::Mutex<rusqlite::Connection>>,
    app_handle: AppHandle,
    /// Leads a lead-finder job inserted
    created_leads: std::sync::Mutex<Vec<i64>>,
}

impl CompletionHandler {
    pub fn new(
        db_conn: Arc<std::sync::Mutex<rusqlite::Connection>>,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            db_conn,
            app_handle,
            created_leads: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Leads inserted by the processed job, for a lead-finder job that completed
    pub fn created_leads(&self) -> Vec<i64> {
        self.created_leads
            .lock()
            .map(|ids| ids.clone())
            .unwrap_or_default()
    }

    /// Process job completion through all phases
    pub fn process_completion(
        &self,
        ctx: &CompletionContext,
        metadata: &JobMetadata,
    ) -> Result<(), CompletionError> {
        // Update completion state: started
        self.update_completion_state(&ctx.job_id, CompletionPhase::Started);

        // If job failed, mark entity as failed and return early
        if !ctx.success {
            self.mark_entity_failed(metadata);
            self.update_completion_state(&ctx.job_id, CompletionPhase::Failed);
            return Ok(());
        }

        // Phase 1: Verify output files
        let outputs = self.verify_output_files(metadata)?;
        self.update_completion_state(&ctx.job_id, CompletionPhase::FilesVerified);

        // Phase 2: Parse and validate content
        let parsed = self.parse_output_content(&outputs, metadata)?;
        self.update_completion_state(&ctx.job_id, CompletionPhase::ContentParsed);

        // Phase 3: Update database
        self.update_database(&parsed, metadata)?;
        self.update_completion_state(&ctx.job_id, CompletionPhase::DatabaseUpdated);

        // Phase 4: Cleanup files (only after DB confirmed)
        self.cleanup_files(metadata)?;
        self.update_completion_state(&ctx.job_id, CompletionPhase::FilesCleanedUp);

        // Phase 5: Emit events and mark complete
        self.emit_completion_events(metadata);
        self.update_completion_state(&ctx.job_id, CompletionPhase::Completed);

        Ok(())
    }

    /// Verify that output files exist and are readable
    fn verify_output_files(
        &self,
        metadata: &JobMetadata,
    ) -> Result<VerifiedOutputs, CompletionError> {
        let primary_path = &metadata.primary_output_path;

        // Company research is asked for the people first and a short summary last. The
        // people are the result, so a missing summary doesn't throw them away.
        let people_written = metadata.job_type == JobType::CompanyResearch
            && metadata
                .secondary_output_path
                .as_ref()
                .is_some_and(|path| path.exists());

        let primary_content = if !primary_path.exists() {
            if !people_written {
                return Err(CompletionError::FileNotFound(primary_path.clone()));
            }
            String::new()
        } else {
            fs::read_to_string(primary_path)
                .map_err(|e| CompletionError::FileReadError(primary_path.clone(), e))?
        };
        let primary_content = if !primary_content.trim().is_empty() {
            primary_content
        } else if people_written {
            "No summary was written for this company.".to_string()
        } else {
            return Err(CompletionError::ValidationError(format!(
                "Primary output file is empty: {:?}",
                primary_path
            )));
        };

        // Read secondary content if path provided
        let secondary_content = if let Some(secondary_path) = &metadata.secondary_output_path {
            if secondary_path.exists() {
                Some(
                    fs::read_to_string(secondary_path)
                        .map_err(|e| CompletionError::FileReadError(secondary_path.clone(), e))?,
                )
            } else {
                None
            }
        } else {
            None
        };

        // Read enrichment content if path provided (optional, don't fail if missing)
        let enrichment_content = if let Some(enrichment_path) = &metadata.enrichment_output_path {
            if enrichment_path.exists() {
                match fs::read_to_string(enrichment_path) {
                    Ok(content) => Some(content),
                    Err(e) => {
                        eprintln!(
                            "[completion_handler] Warning: Failed to read enrichment file {:?}: {}",
                            enrichment_path, e
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(VerifiedOutputs {
            primary_content,
            secondary_content,
            enrichment_content,
        })
    }

    /// Parse output content based on job type
    fn parse_output_content(
        &self,
        outputs: &VerifiedOutputs,
        metadata: &JobMetadata,
    ) -> Result<ParsedOutput, CompletionError> {
        match metadata.job_type {
            JobType::CompanyResearch => {
                let people = if let Some(ref people_json) = outputs.secondary_content {
                    match serde_json::from_str::<Vec<serde_json::Value>>(people_json) {
                        Ok(p) => Some(p),
                        Err(e) => {
                            eprintln!(
                                "[completion_handler] Warning: Failed to parse people JSON: {}",
                                e
                            );
                            None
                        }
                    }
                } else {
                    None
                };

                // Parse enrichment data (optional)
                let enrichment = if let Some(ref enrichment_json) = outputs.enrichment_content {
                    match serde_json::from_str::<LeadEnrichment>(enrichment_json) {
                        Ok(e) => Some(e),
                        Err(e) => {
                            eprintln!("[completion_handler] Warning: Failed to parse lead enrichment JSON: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                Ok(ParsedOutput::CompanyResearch {
                    profile: outputs.primary_content.clone(),
                    people,
                    enrichment,
                })
            }
            JobType::PersonResearch => {
                // Parse enrichment data (optional)
                let enrichment = if let Some(ref enrichment_json) = outputs.enrichment_content {
                    match serde_json::from_str::<PersonEnrichment>(enrichment_json) {
                        Ok(e) => Some(e),
                        Err(e) => {
                            eprintln!("[completion_handler] Warning: Failed to parse person enrichment JSON: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                Ok(ParsedOutput::PersonResearch {
                    profile: outputs.primary_content.clone(),
                    enrichment,
                })
            }
            JobType::Scoring => {
                let score_data: serde_json::Value = serde_json::from_str(&outputs.primary_content)
                    .map_err(|e| {
                        CompletionError::ParseError(format!("Invalid score JSON: {}", e))
                    })?;

                // Validate required fields
                if score_data.get("passesRequirements").is_none() {
                    return Err(CompletionError::ValidationError(
                        "Score JSON missing 'passesRequirements' field".to_string(),
                    ));
                }

                Ok(ParsedOutput::Scoring { score_data })
            }
            JobType::Conversation => Ok(ParsedOutput::Conversation {
                topics: outputs.primary_content.clone(),
            }),
            JobType::LeadFinder => {
                let leads: Vec<serde_json::Value> = serde_json::from_str(&outputs.primary_content)
                    .map_err(|e| {
                        CompletionError::ParseError(format!("Invalid leads JSON: {}", e))
                    })?;
                Ok(ParsedOutput::LeadFinder { leads })
            }
        }
    }

    /// Update database with parsed output (wrapped in a transaction for atomicity)
    fn update_database(
        &self,
        parsed: &ParsedOutput,
        metadata: &JobMetadata,
    ) -> Result<(), CompletionError> {
        let mut conn = self.db_conn.lock().map_err(|e| {
            CompletionError::DatabaseError(format!("Failed to lock database: {}", e))
        })?;

        // Start a transaction for atomic updates
        let tx = conn.transaction().map_err(|e| {
            CompletionError::DatabaseError(format!("Failed to start transaction: {}", e))
        })?;

        let result = self.update_database_in_tx(&tx, parsed, metadata);

        match result {
            Ok(()) => {
                tx.commit().map_err(|e| {
                    CompletionError::DatabaseError(format!("Failed to commit transaction: {}", e))
                })?;
                Ok(())
            }
            Err(e) => {
                // Transaction will be rolled back on drop
                eprintln!(
                    "[completion_handler] Transaction rolled back due to error: {}",
                    e
                );
                Err(e)
            }
        }
    }

    /// Internal: update database within a transaction
    fn update_database_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        parsed: &ParsedOutput,
        metadata: &JobMetadata,
    ) -> Result<(), CompletionError> {
        match parsed {
            ParsedOutput::CompanyResearch {
                profile,
                people,
                enrichment,
            } => {
                let lead_id = metadata.entity_id;

                if let Some(people_data) = people {
                    // The company's own domain: the website we were given, or the one
                    // research confirmed
                    let known_website: Option<String> = tx
                        .query_row(
                            "SELECT website FROM leads WHERE id = ?1",
                            rusqlite::params![lead_id],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(|e| CompletionError::DatabaseError(e.to_string()))?
                        .flatten();
                    let site_domain = known_website
                        .as_deref()
                        .or(enrichment.as_ref().and_then(|e| e.website.as_deref()))
                        .and_then(website_domain);
                    store_found_people(
                        tx,
                        lead_id,
                        finalize_people(people_data, enrichment.as_ref(), site_domain.as_deref()),
                        chrono::Utc::now().timestamp(),
                    )
                    .map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                }

                // Update lead with company profile
                let now = chrono::Utc::now().timestamp();
                let rows = tx.execute(
                    "UPDATE leads SET research_status = ?1, company_profile = ?2, researched_at = ?3 WHERE id = ?4",
                    rusqlite::params!["completed", profile, now, lead_id],
                ).map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                if rows == 0 {
                    return Err(CompletionError::ValidationError(format!(
                        "Lead {} no longer exists; company research output discarded",
                        lead_id
                    )));
                }

                // Apply enrichment data if available (only updates NULL fields)
                if let Some(e) = enrichment {
                    db::enrich_lead(tx, lead_id, e)
                        .map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                }
            }
            ParsedOutput::PersonResearch {
                profile,
                enrichment,
            } => {
                let person_id = metadata.entity_id;
                let now = chrono::Utc::now().timestamp();
                let rows = tx
                    .execute(
                        // The note is a few lines. A full profile from an earlier version of
                        // Closer is worth more than that, so it stays.
                        "UPDATE people SET research_status = ?1,
                            person_profile = CASE WHEN LENGTH(COALESCE(person_profile, '')) > 1000
                                                  THEN person_profile ELSE ?2 END,
                            researched_at = ?3
                     WHERE id = ?4",
                        rusqlite::params!["completed", profile, now, person_id],
                    )
                    .map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                if rows == 0 {
                    return Err(CompletionError::ValidationError(format!(
                        "Person {} no longer exists; research output discarded",
                        person_id
                    )));
                }

                // Apply enrichment data if available (only updates NULL fields)
                if let Some(e) = enrichment {
                    db::enrich_person(tx, person_id, e)
                        .map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                }
            }
            ParsedOutput::Scoring { score_data } => {
                let lead_id = metadata.entity_id;

                // Get active config (read operation, safe outside transaction scope)
                let config: db::ParsedScoringConfig = tx
                    .query_row(
                        "SELECT id, name, is_active, required_characteristics, demand_signifiers,
                            tier_hot_min, tier_warm_min, tier_nurture_min, created_at, updated_at
                     FROM scoring_config WHERE is_active = 1 ORDER BY id DESC LIMIT 1",
                        [],
                        |row| {
                            let required_chars: String = row.get(3)?;
                            let demand_sigs: String = row.get(4)?;
                            Ok(db::ParsedScoringConfig {
                                id: row.get(0)?,
                                name: row.get(1)?,
                                is_active: row.get(2)?,
                                required_characteristics: serde_json::from_str(&required_chars)
                                    .unwrap_or(serde_json::Value::Array(vec![])),
                                demand_signifiers: serde_json::from_str(&demand_sigs)
                                    .unwrap_or(serde_json::Value::Array(vec![])),
                                tier_hot_min: row.get(5)?,
                                tier_warm_min: row.get(6)?,
                                tier_nurture_min: row.get(7)?,
                                created_at: row.get(8)?,
                                updated_at: row.get(9)?,
                            })
                        },
                    )
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => CompletionError::ValidationError(
                            "No active scoring configuration".to_string(),
                        ),
                        _ => CompletionError::DatabaseError(e.to_string()),
                    })?;

                let passes_requirements = score_data
                    .get("passesRequirements")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let requirement_results = score_data
                    .get("requirementResults")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "[]".to_string());

                let mut total_score = score_data.get("totalScore")
                    .and_then(|v| {
                        // Try as float first (handles both integer and float JSON values)
                        v.as_f64()
                            .map(|f| f.round() as i64)
                            .or_else(|| v.as_i64()) // Fallback to direct integer parsing
                    })
                    .unwrap_or_else(|| {
                        eprintln!("[completion_handler] Warning: totalScore missing or invalid in score data for lead {}, defaulting to 0", lead_id);
                        0
                    });

                // Defensive recalculation: if totalScore is 0 but requirements passed, recalculate from breakdown
                if total_score == 0 && passes_requirements {
                    if let Some(breakdown) =
                        score_data.get("scoreBreakdown").and_then(|v| v.as_array())
                    {
                        if !breakdown.is_empty() {
                            let calculated_score: f64 = breakdown
                                .iter()
                                .filter_map(|item| {
                                    let score = item.get("score").and_then(|v| v.as_f64())?;
                                    let weight = item.get("weight").and_then(|v| v.as_f64())?;
                                    Some(score * weight)
                                })
                                .sum();

                            let total_weight: f64 = breakdown
                                .iter()
                                .filter_map(|item| item.get("weight").and_then(|v| v.as_f64()))
                                .sum();

                            if total_weight > 0.0 {
                                let recalculated = (calculated_score / total_weight).round() as i64;
                                if recalculated > 0 {
                                    eprintln!("[completion_handler] Recalculated totalScore from breakdown: {} (was 0) for lead {}",
                                             recalculated, lead_id);
                                    total_score = recalculated;
                                }
                            }
                        }
                    }
                }

                let score_breakdown = score_data
                    .get("scoreBreakdown")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "[]".to_string());

                let tier = score_data
                    .get("tier")
                    .and_then(|v| v.as_str())
                    .unwrap_or("disqualified")
                    .to_string();

                let scoring_notes = score_data.get("scoringNotes").and_then(|v| v.as_str());

                // Enhanced logging for score parsing verification
                eprintln!("[completion_handler] Parsed score for lead {}: passesRequirements={}, totalScore={}, tier={}",
                         lead_id, passes_requirements, total_score, tier);

                let now = chrono::Utc::now().timestamp();

                // Delete existing scores for this lead
                tx.execute(
                    "DELETE FROM lead_scores WHERE lead_id = ?1",
                    rusqlite::params![lead_id],
                )
                .map_err(|e| CompletionError::DatabaseError(e.to_string()))?;

                // Insert new score
                tx.execute(
                    "INSERT INTO lead_scores (lead_id, config_id, passes_requirements, requirement_results,
                     total_score, score_breakdown, tier, scoring_notes, scored_at, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![lead_id, config.id, passes_requirements, requirement_results,
                                      total_score, score_breakdown, tier, scoring_notes, now, now],
                ).map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
            }
            ParsedOutput::Conversation { topics } => {
                let person_id = metadata.entity_id;
                let now = chrono::Utc::now().timestamp();
                let rows = tx.execute(
                    "UPDATE people SET conversation_topics = ?1, conversation_generated_at = ?2 WHERE id = ?3",
                    rusqlite::params![topics, now, person_id],
                ).map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                if rows == 0 {
                    return Err(CompletionError::ValidationError(format!(
                        "Person {} no longer exists; conversation output discarded",
                        person_id
                    )));
                }
            }
            ParsedOutput::LeadFinder { leads } => {
                let now = chrono::Utc::now().timestamp();

                // Skip companies we already have, matched by name or website domain
                let mut known_names = HashSet::new();
                let mut known_domains = HashSet::new();
                {
                    let mut stmt = tx
                        .prepare("SELECT company_name, website FROM leads")
                        .map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                    let rows = stmt
                        .query_map([], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                        })
                        .map_err(|e| CompletionError::DatabaseError(e.to_string()))?;
                    for (name, website) in rows.flatten() {
                        known_names.insert(name.trim().to_lowercase());
                        if let Some(domain) = website.as_deref().and_then(website_domain) {
                            known_domains.insert(domain);
                        }
                    }
                }

                for lead_data in leads {
                    let company_name = lead_data
                        .get("companyName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let website = lead_data.get("website").and_then(|v| v.as_str());
                    let domain = website.and_then(website_domain);
                    let is_new_name = known_names.insert(company_name.trim().to_lowercase());
                    let is_new_domain = domain.is_none_or(|d| known_domains.insert(d));
                    if !is_new_name || !is_new_domain {
                        eprintln!(
                            "[completion_handler] Skipping {} (already in the lead list)",
                            company_name
                        );
                        continue;
                    }
                    let city = lead_data.get("city").and_then(|v| v.as_str());
                    let state = lead_data.get("state").and_then(|v| v.as_str());
                    let country = lead_data.get("country").and_then(|v| v.as_str());
                    let industry = lead_data.get("industry").and_then(|v| v.as_str());

                    tx.execute(
                        "INSERT INTO leads (company_name, website, city, state, country, industry, research_status, user_status, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 'new', ?7)",
                        rusqlite::params![company_name, website, city, state, country, industry, now],
                    ).map_err(|e| CompletionError::DatabaseError(e.to_string()))?;

                    let lead_id = tx.last_insert_rowid();
                    if let Ok(mut created) = self.created_leads.lock() {
                        created.push(lead_id);
                    }
                    // Emit lead-created event so frontend updates incrementally
                    events::emit_lead_created(&self.app_handle, lead_id);
                }
            }
        }

        Ok(())
    }

    /// Cleanup output files after database is confirmed updated
    fn cleanup_files(&self, metadata: &JobMetadata) -> Result<(), CompletionError> {
        let primary_path = &metadata.primary_output_path;

        // For research jobs, delete the entire directory
        if matches!(
            metadata.job_type,
            JobType::CompanyResearch | JobType::PersonResearch
        ) {
            if let Some(parent) = primary_path.parent() {
                if parent.exists() {
                    if let Err(e) = fs::remove_dir_all(parent) {
                        eprintln!(
                            "[completion_handler] Warning: Failed to cleanup directory {:?}: {}",
                            parent, e
                        );
                    }
                }
            }
        } else {
            // For other jobs, just delete the output file
            if primary_path.exists() {
                if let Err(e) = fs::remove_file(primary_path) {
                    eprintln!(
                        "[completion_handler] Warning: Failed to cleanup file {:?}: {}",
                        primary_path, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Emit completion events for frontend cache invalidation
    fn emit_completion_events(&self, metadata: &JobMetadata) {
        match metadata.job_type {
            JobType::CompanyResearch => {
                events::emit_lead_updated(&self.app_handle, metadata.entity_id);
                events::emit_people_bulk_created(&self.app_handle, metadata.entity_id);
            }
            JobType::PersonResearch => {
                // Get lead_id for the person
                if let Ok(conn) = self.db_conn.lock() {
                    if let Ok(Some(person)) = db::get_person_raw(&conn, metadata.entity_id) {
                        events::emit_person_updated(
                            &self.app_handle,
                            metadata.entity_id,
                            person.lead_id,
                        );
                    }
                }
            }
            JobType::Scoring => {
                events::emit_lead_scored(&self.app_handle, metadata.entity_id);
            }
            JobType::Conversation => {
                if let Ok(conn) = self.db_conn.lock() {
                    if let Ok(Some(person)) = db::get_person_raw(&conn, metadata.entity_id) {
                        events::emit_person_updated(
                            &self.app_handle,
                            metadata.entity_id,
                            person.lead_id,
                        );
                    }
                }
            }
            JobType::LeadFinder => {
                // Events already emitted per-lead during insert in update_database_in_tx
            }
        }
    }

    /// Mark entity as failed when job fails
    pub fn mark_entity_failed(&self, metadata: &JobMetadata) {
        if let Ok(conn) = self.db_conn.lock() {
            match metadata.job_type {
                JobType::CompanyResearch => {
                    let _ = conn.execute(
                        "UPDATE leads SET research_status = 'failed' WHERE id = ?1",
                        rusqlite::params![metadata.entity_id],
                    );
                }
                JobType::PersonResearch => {
                    let _ = conn.execute(
                        "UPDATE people SET research_status = 'failed' WHERE id = ?1",
                        rusqlite::params![metadata.entity_id],
                    );
                }
                JobType::Scoring | JobType::Conversation | JobType::LeadFinder => {
                    // No status field to update for these types
                }
            }
        }
    }

    /// Update completion state in database for recovery
    fn update_completion_state(&self, job_id: &str, phase: CompletionPhase) {
        let state_str = serde_json::to_string(&phase).ok();
        if let Ok(conn) = self.db_conn.lock() {
            let _ = db::update_job_completion_state(&conn, job_id, state_str.as_deref());
        }
    }
}

/// Save the people company research found for a lead. People are matched by
/// (lead_id, first_name, last_name) so existing IDs, user edits and earlier research survive a
/// re-run: only empty fields are filled.
///
/// Someone who arrives with an email is done, there being nothing else to research. Someone
/// without one stays pending, which is what "Find missing emails" picks up.
fn store_found_people(
    conn: &rusqlite::Connection,
    lead_id: i64,
    people: Vec<FoundPerson>,
    now: i64,
) -> rusqlite::Result<()> {
    for person in people {
        let FoundPerson {
            first_name,
            last_name,
            title,
            linkedin_url,
            management_level,
            year_joined,
            email,
            email_source,
            email_evidence,
        } = person;
        let note = email
            .as_deref()
            .map(|email| email_note(email, email_source, email_evidence.as_deref()));

        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM people WHERE lead_id = ?1 AND first_name = ?2 AND last_name = ?3 LIMIT 1",
                rusqlite::params![lead_id, first_name, last_name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing_id {
            // ?1 fills the email only when the person had none; the source, note and
            // "completed" follow that same condition so they always describe this email
            conn.execute(
                "UPDATE people SET
                    email_source = CASE WHEN NULLIF(TRIM(email), '') IS NULL AND ?1 IS NOT NULL
                                        THEN ?7 ELSE email_source END,
                    person_profile = CASE WHEN NULLIF(TRIM(email), '') IS NULL AND ?1 IS NOT NULL
                                          THEN COALESCE(person_profile, ?8) ELSE person_profile END,
                    research_status = CASE WHEN NULLIF(TRIM(email), '') IS NULL AND ?1 IS NOT NULL
                                                AND COALESCE(research_status, 'pending') IN ('pending', 'failed')
                                           THEN 'completed' ELSE research_status END,
                    researched_at = CASE WHEN NULLIF(TRIM(email), '') IS NULL AND ?1 IS NOT NULL
                                         THEN COALESCE(researched_at, ?9) ELSE researched_at END,
                    email = COALESCE(NULLIF(TRIM(email), ''), ?1),
                    title = COALESCE(title, ?2),
                    linkedin_url = COALESCE(linkedin_url, ?3),
                    management_level = COALESCE(management_level, ?4),
                    year_joined = COALESCE(year_joined, ?5)
                 WHERE id = ?6",
                rusqlite::params![
                    email,
                    title,
                    linkedin_url,
                    management_level,
                    year_joined,
                    id,
                    email_source,
                    note,
                    now
                ],
            )?;
        } else {
            let (status, researched_at) = if email.is_some() {
                ("completed", Some(now))
            } else {
                ("pending", None)
            };
            conn.execute(
                "INSERT INTO people (first_name, last_name, email, email_source, person_profile, title, linkedin_url, management_level, year_joined, lead_id, research_status, researched_at, user_status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'new', ?13)",
                rusqlite::params![
                    first_name,
                    last_name,
                    email,
                    email_source,
                    note,
                    title,
                    linkedin_url,
                    management_level,
                    year_joined,
                    lead_id,
                    status,
                    researched_at,
                    now
                ],
            )?;
        }
    }
    Ok(())
}

/// What a person's page says about an email that company research found
fn email_note(email: &str, source: Option<&str>, evidence: Option<&str>) -> String {
    let evidence = evidence.map(str::trim).filter(|text| !text.is_empty());
    let how = match (source, evidence) {
        (Some("pattern"), Some(evidence)) => {
            format!("From the company's pattern, not seen published. Based on: {evidence}")
        }
        (Some("pattern"), None) => "From the company's pattern, not seen published.".to_string(),
        (_, Some(evidence)) => format!("Published: {evidence}"),
        (_, None) => "Found during company research.".to_string(),
    };
    format!("## Email\n\n{email}\n\n{how}\n")
}

/// "https://www.Acme.com/about" -> "acme.com"
fn website_domain(website: &str) -> Option<String> {
    let rest = website.trim().to_lowercase();
    let rest = rest.split("://").last().unwrap_or_default();
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.strip_prefix("www.").unwrap_or(host);
    (!host.is_empty()).then(|| host.to_string())
}

#[cfg(test)]
mod tests {
    use super::{email_note, store_found_people, website_domain};
    use crate::jobs::found_people::finalize_people;
    use rusqlite::Connection;
    use serde_json::json;

    /// (email, email_source, research_status, has a note) for a person
    fn row(conn: &Connection, first: &str) -> (Option<String>, Option<String>, String, bool) {
        conn.query_row(
            "SELECT email, email_source, research_status, person_profile IS NOT NULL
             FROM people WHERE first_name = ?1",
            [first],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
    }

    #[test]
    fn stores_found_people_and_keeps_what_was_already_there() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO leads (id, company_name, created_at) VALUES (7, 'Acme', 1)",
            [],
        )
        .unwrap();
        // Typed in by the user, with their own email; and one found earlier with no email
        conn.execute_batch(
            "INSERT INTO people (lead_id, first_name, last_name, email, title, research_status, user_status, created_at)
             VALUES (7, 'Uma', 'Typed', 'uma@own.com', 'Owner', 'pending', 'new', 1);
             INSERT INTO people (lead_id, first_name, last_name, research_status, user_status, created_at)
             VALUES (7, 'Old', 'Timer', 'pending', 'new', 1);",
        )
        .unwrap();

        let people = [
            json!({"firstName": "Kizzy", "lastName": "Lewis", "title": "HR", "email": "klewis@acme.org", "emailSource": "published", "emailEvidence": "https://acme.org/team"}),
            json!({"firstName": "Michael", "lastName": "Lucas"}),
            json!({"firstName": "Jo", "lastName": "Williams-Neal"}),
            json!({"firstName": "Uma", "lastName": "Typed", "email": "utyped@acme.org", "emailSource": "published", "title": "CEO"}),
            json!({"firstName": "Old", "lastName": "Timer"}),
        ];
        store_found_people(
            &conn,
            7,
            finalize_people(&people, None, Some("acme.org")),
            100,
        )
        .unwrap();

        let done = "completed".to_string();
        let pending = "pending".to_string();
        assert_eq!(
            row(&conn, "Kizzy"),
            (
                Some("klewis@acme.org".into()),
                Some("published".into()),
                done.clone(),
                true
            )
        );
        // The pattern two published addresses agree on, applied and marked as such
        assert_eq!(
            row(&conn, "Michael"),
            (
                Some("mlucas@acme.org".into()),
                Some("pattern".into()),
                done.clone(),
                true
            )
        );
        // No email: waits for "Find missing emails"
        assert_eq!(row(&conn, "Jo"), (None, None, pending.clone(), false));
        // The user's own address and title win, and aren't labelled as research's
        assert_eq!(
            row(&conn, "Uma"),
            (Some("uma@own.com".into()), None, pending, false)
        );
        let title: String = conn
            .query_row(
                "SELECT title FROM people WHERE first_name = 'Uma'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Owner");
        // Someone already listed gets the email this run worked out
        assert_eq!(
            row(&conn, "Old"),
            (
                Some("otimer@acme.org".into()),
                Some("pattern".into()),
                done,
                true
            )
        );

        // Running again adds nobody and changes nothing
        store_found_people(
            &conn,
            7,
            finalize_people(&people, None, Some("acme.org")),
            200,
        )
        .unwrap();
        let (count, researched): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), MAX(researched_at) FROM people WHERE lead_id = 7",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((count, researched), (5, 100));
    }

    #[test]
    fn email_note_says_how_the_address_is_known() {
        assert_eq!(
            email_note(
                "jane@acme.org",
                Some("published"),
                Some("https://acme.org/team")
            ),
            "## Email\n\njane@acme.org\n\nPublished: https://acme.org/team\n"
        );
        assert!(
            email_note("jane@acme.org", Some("pattern"), Some("bob.lee@acme.org")).contains(
                "From the company's pattern, not seen published. Based on: bob.lee@acme.org"
            )
        );
        assert!(email_note("jane@acme.org", None, Some("  "))
            .contains("Found during company research."));
    }

    #[test]
    fn website_domain_normalizes_urls() {
        assert_eq!(
            website_domain("https://www.Acme.com/about").as_deref(),
            Some("acme.com")
        );
        assert_eq!(website_domain("acme.com").as_deref(), Some("acme.com"));
        assert_eq!(
            website_domain("http://shop.acme.co.uk?ref=x").as_deref(),
            Some("shop.acme.co.uk")
        );
        assert_eq!(website_domain("  "), None);
    }
}
