//! Autopilot: when a research step finishes, start the next one, so nobody has to open
//! each lead and click through company research, scoring, people and talking points.
//!
//! Find leads → research each new company → score it and research its new people →
//! talking points for each person. Turned off with the `auto_research` setting.

use crate::commands::{
    queue_company_research, queue_conversation, queue_person_research, queue_scoring,
};
use crate::db::{self, DbState};
use crate::jobs::{JobMetadata, JobType, StreamEvent};
use rusqlite::{params, Connection};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager};

enum NextStep {
    ResearchLead(i64),
    Score(i64),
    ResearchPerson(i64),
    Conversation(i64),
}

/// Streams a background job's output to the UI as `job-stream` events, since no page
/// holds a channel for it
fn background_channel(app: &AppHandle) -> Channel<StreamEvent> {
    let app = app.clone();
    Channel::new(move |body| {
        if let InvokeResponseBody::Json(json) = body {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&json) {
                let _ = app.emit("job-stream", event);
            }
        }
        Ok(())
    })
}

/// Queue the follow-up jobs for a job that finished successfully
pub fn job_succeeded(app: &AppHandle, job_id: &str, metadata: &JobMetadata) {
    let steps = match next_steps(app, job_id, metadata) {
        Ok(steps) => steps,
        Err(e) => {
            eprintln!("[autopilot] Couldn't plan the steps after job {job_id}: {e}");
            return;
        }
    };
    if steps.is_empty() {
        return;
    }
    eprintln!(
        "[autopilot] Job {job_id} done, queueing {} follow-up jobs",
        steps.len()
    );

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for step in steps {
            let on_event = background_channel(&app);
            let result = match step {
                NextStep::ResearchLead(id) => {
                    queue_company_research(app.clone(), id, None, on_event, true).await
                }
                NextStep::Score(id) => queue_scoring(app.clone(), id, on_event, true).await,
                NextStep::ResearchPerson(id) => {
                    queue_person_research(app.clone(), id, None, on_event, true).await
                }
                NextStep::Conversation(id) => {
                    queue_conversation(app.clone(), id, on_event, true).await
                }
            };
            if let Err(e) = result {
                eprintln!("[autopilot] Couldn't queue a follow-up job: {e}");
            }
        }
    });
}

fn next_steps(
    app: &AppHandle,
    job_id: &str,
    metadata: &JobMetadata,
) -> Result<Vec<NextStep>, String> {
    let state = app.state::<DbState>();
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    if !db::get_settings(&conn)
        .map_err(|e| e.to_string())?
        .auto_research
    {
        return Ok(Vec::new());
    }

    let id = metadata.entity_id;
    let steps = match metadata.job_type {
        JobType::LeadFinder => leads_found_by(&conn, job_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(NextStep::ResearchLead)
            .collect(),
        JobType::CompanyResearch => {
            let mut steps = Vec::new();
            if db::get_active_scoring_config(&conn)
                .map_err(|e| e.to_string())?
                .is_some()
            {
                steps.push(NextStep::Score(id));
            }
            // Only people this research added; ones already researched keep their profile
            steps.extend(
                unresearched_people(&conn, id)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .map(NextStep::ResearchPerson),
            );
            steps
        }
        JobType::PersonResearch => vec![NextStep::Conversation(id)],
        JobType::Scoring | JobType::Conversation => Vec::new(),
    };
    Ok(steps)
}

/// Leads a find-leads job added, which are the unresearched ones created since it started
fn leads_found_by(conn: &Connection, job_id: &str) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM leads
         WHERE research_status = 'pending'
           AND created_at >= (SELECT created_at FROM jobs WHERE id = ?1)
         ORDER BY id",
    )?;
    let ids = stmt.query_map(params![job_id], |row| row.get(0))?;
    ids.collect()
}

fn unresearched_people(conn: &Connection, lead_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM people
         WHERE lead_id = ?1 AND COALESCE(research_status, 'pending') = 'pending'
         ORDER BY id",
    )?;
    let ids = stmt.query_map(params![lead_id], |row| row.get(0))?;
    ids.collect()
}

#[cfg(test)]
mod tests {
    use super::{leads_found_by, unresearched_people};
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        conn
    }

    fn add_lead(conn: &Connection, name: &str, status: &str, created_at: i64) -> i64 {
        conn.execute(
            "INSERT INTO leads (company_name, research_status, user_status, created_at)
             VALUES (?1, ?2, 'new', ?3)",
            rusqlite::params![name, status, created_at],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn lead_finder_follow_up_researches_only_its_new_leads() {
        let conn = db();
        add_lead(&conn, "Old pending", "pending", 100);
        conn.execute(
            "INSERT INTO jobs (id, job_type, entity_id, entity_label, status, prompt, working_dir, created_at)
             VALUES ('job', 'lead_finder', 0, 'Find', 'completed', '', '.', 200)",
            [],
        )
        .unwrap();
        let found = add_lead(&conn, "Found", "pending", 250);
        add_lead(&conn, "Researched meanwhile", "completed", 260);

        assert_eq!(leads_found_by(&conn, "job").unwrap(), vec![found]);
    }

    #[test]
    fn company_follow_up_skips_people_already_researched() {
        let conn = db();
        let lead = add_lead(&conn, "Acme", "completed", 100);
        let other = add_lead(&conn, "Other", "completed", 100);
        let add_person = |lead_id: i64, status: &str| {
            conn.execute(
                "INSERT INTO people (first_name, last_name, lead_id, research_status, user_status, created_at)
                 VALUES ('A', 'B', ?1, ?2, 'new', 100)",
                rusqlite::params![lead_id, status],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        let new_person = add_person(lead, "pending");
        add_person(lead, "completed");
        add_person(other, "pending");

        assert_eq!(unresearched_people(&conn, lead).unwrap(), vec![new_person]);
    }
}
