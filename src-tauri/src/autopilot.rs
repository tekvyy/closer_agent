//! Autopilot: when Find Leads finishes, research each company it found, so nobody has to
//! open every lead and click Research.
//!
//! That is the whole chain. One research session per company brings its people and their
//! emails; scoring, a deeper search for one person's email and talking points only run when
//! someone asks for them, because each is another AI session against the account's limit.

use crate::commands::queue_company_research;
use crate::jobs::{JobMetadata, JobType, StreamEvent};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter};

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

/// Queue the follow-up jobs for a job that finished successfully. `created_leads` are
/// the leads a find-leads job inserted.
pub fn job_succeeded(app: &AppHandle, metadata: &JobMetadata, created_leads: &[i64]) {
    let leads = leads_to_research(metadata.job_type, created_leads);
    if leads.is_empty() {
        return;
    }
    eprintln!(
        "[autopilot] Queueing research for {} new leads",
        leads.len()
    );

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for lead_id in leads {
            let on_event = background_channel(&app);
            if let Err(e) = queue_company_research(app.clone(), lead_id, None, on_event, true).await
            {
                eprintln!("[autopilot] Couldn't queue research for lead {lead_id}: {e}");
            }
        }
    });
}

fn leads_to_research(job_type: JobType, created_leads: &[i64]) -> Vec<i64> {
    match job_type {
        JobType::LeadFinder => created_leads.to_vec(),
        JobType::CompanyResearch
        | JobType::PersonResearch
        | JobType::Scoring
        | JobType::Conversation => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::leads_to_research;
    use crate::jobs::JobType;

    #[test]
    fn only_find_leads_starts_more_work() {
        assert_eq!(leads_to_research(JobType::LeadFinder, &[4, 9]), vec![4, 9]);
        for job_type in [
            JobType::CompanyResearch,
            JobType::PersonResearch,
            JobType::Scoring,
            JobType::Conversation,
        ] {
            assert!(leads_to_research(job_type, &[4, 9]).is_empty());
        }
    }
}
