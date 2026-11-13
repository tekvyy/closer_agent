//! Push to CRM: send the people picked in the People list to a CRM that accepts them
//! (a CRM that offers `/api/research/people`), using a connection key made in that CRM.
//!
//! The key lives in the local database next to everything else the app stores, and is
//! never sent back to the window: the frontend only learns whether a connection exists.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

use crate::db::DbState;

const API_PATH: &str = "/api/research/people";
/// The CRM refuses more than this in one request
const BATCH_SIZE: usize = 200;
const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrmConnection {
    pub url: String,
    pub key_name: String,
    pub connected_at: i64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PushSummary {
    pub created: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub archived: u32,
    pub invalid: u32,
    /// People with no email address, so they weren't sent
    pub no_email: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutgoingPerson {
    email: String,
    first_name: String,
    last_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    company: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    job_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    linkedin_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
}

#[derive(Deserialize)]
struct CheckResponse {
    #[serde(rename = "keyName")]
    key_name: String,
}

#[derive(Deserialize)]
struct PushResponse {
    counts: PushSummary,
}

#[derive(Deserialize)]
struct ErrorResponse {
    message: Option<String>,
}

pub fn create_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS crm_connection (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            url TEXT NOT NULL,
            api_key TEXT NOT NULL,
            key_name TEXT NOT NULL,
            connected_at INTEGER NOT NULL
        );",
    )
}

/// The CRM's address as typed or pasted: `crm.example.com`, `https://crm.example.com/admin/` and
/// `https://crm.example.com` all become `https://crm.example.com`. Plain http only for this computer.
fn normalise_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim().trim_end_matches('/');
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let (scheme, rest) = with_scheme
        .split_once("://")
        .ok_or("That address doesn't look right.")?;
    let host_and_port = rest.split('/').next().unwrap_or("");
    let host = host_and_port.split(':').next().unwrap_or("");
    if host.is_empty() || !host.contains('.') && host != "localhost" {
        return Err("That address doesn't look right. It should be like crm.example.com.".into());
    }
    let local = host == "localhost" || host == "127.0.0.1";
    if scheme == "http" && !local {
        return Err("The CRM address has to start with https://".into());
    }
    Ok(format!("{scheme}://{host_and_port}"))
}

fn client() -> Result<reqwest::Client, String> {
    // reqwest is built without a TLS provider of its own; the updater installs ring the same way
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| e.to_string())
}

/// Turns a failed response into words for the person using the app
async fn explain(response: reqwest::Response) -> String {
    let status = response.status();
    let message = response
        .json::<ErrorResponse>()
        .await
        .ok()
        .and_then(|body| body.message);
    match status.as_u16() {
        401 => "The CRM didn't accept this connection key. Make a new one in the CRM (Settings, Research tool) and connect again.".into(),
        404 => "The CRM at this address can't receive contacts yet. Check the address.".into(),
        429 => "The CRM is getting too many requests. Wait a few minutes and try again.".into(),
        _ => message.unwrap_or_else(|| format!("The CRM answered with an error ({status}). Try again.")),
    }
}

fn read_connection(conn: &Connection) -> rusqlite::Result<Option<(CrmConnection, String)>> {
    conn.query_row(
        "SELECT url, api_key, key_name, connected_at FROM crm_connection WHERE id = 1",
        [],
        |row| {
            Ok((
                CrmConnection {
                    url: row.get(0)?,
                    key_name: row.get(2)?,
                    connected_at: row.get(3)?,
                },
                row.get::<_, String>(1)?,
            ))
        },
    )
    .optional()
}

#[tauri::command]
pub fn get_crm_connection(state: State<'_, DbState>) -> Result<Option<CrmConnection>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    Ok(read_connection(&conn)
        .map_err(|e| e.to_string())?
        .map(|(c, _)| c))
}

/// Checks the address and key with the CRM, and saves them only if the CRM accepts them
#[tauri::command]
pub async fn connect_crm(
    state: State<'_, DbState>,
    url: String,
    key: String,
) -> Result<CrmConnection, String> {
    let url = normalise_url(&url)?;
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Paste the connection key from the CRM.".into());
    }

    let response = client()?
        .get(format!("{url}{API_PATH}"))
        .bearer_auth(&key)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach the CRM at {url}. Check the address and your internet connection. ({e})"))?;
    if !response.status().is_success() {
        return Err(explain(response).await);
    }
    let checked: CheckResponse = response.json().await.map_err(|_| {
        "That address answered, but it isn't a CRM that accepts contacts. Check the address."
            .to_string()
    })?;

    let connection = CrmConnection {
        url,
        key_name: checked.key_name,
        connected_at: chrono::Utc::now().timestamp_millis(),
    };
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO crm_connection (id, url, api_key, key_name, connected_at) VALUES (1, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET url = excluded.url, api_key = excluded.api_key,
            key_name = excluded.key_name, connected_at = excluded.connected_at",
        params![connection.url, key, connection.key_name, connection.connected_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(connection)
}

#[tauri::command]
pub fn disconnect_crm(state: State<'_, DbState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM crm_connection", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn load_people(
    conn: &Connection,
    ids: &[i64],
) -> rusqlite::Result<Vec<(Option<String>, OutgoingPerson)>> {
    let mut stmt = conn.prepare(
        "SELECT p.email, p.first_name, p.last_name, p.title, p.linkedin_url, p.person_profile, l.company_name
         FROM people p LEFT JOIN leads l ON p.lead_id = l.id WHERE p.id = ?1",
    )?;
    let mut people = Vec::with_capacity(ids.len());
    for id in ids {
        let row = stmt
            .query_row(params![id], |row| {
                let email: Option<String> = row.get(0)?;
                Ok((
                    non_empty(email.clone()),
                    OutgoingPerson {
                        email: email.unwrap_or_default().trim().to_string(),
                        first_name: row.get(1)?,
                        last_name: row.get(2)?,
                        job_title: non_empty(row.get(3)?),
                        linkedin_url: non_empty(row.get(4)?),
                        summary: non_empty(row.get(5)?),
                        company: non_empty(row.get(6)?),
                    },
                ))
            })
            .optional()?;
        if let Some(person) = row {
            people.push(person);
        }
    }
    Ok(people)
}

/// Sends the chosen people in batches. A batch that fails stops the push; batches already
/// sent stay in the CRM, and sending them again is harmless because the CRM only fills blanks.
#[tauri::command]
pub async fn push_people_to_crm(
    state: State<'_, DbState>,
    person_ids: Vec<i64>,
) -> Result<PushSummary, String> {
    let (connection, key, people) = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let Some((connection, key)) = read_connection(&conn).map_err(|e| e.to_string())? else {
            return Err("Connect to your CRM first.".into());
        };
        let people = load_people(&conn, &person_ids).map_err(|e| e.to_string())?;
        (connection, key, people)
    };

    let mut summary = PushSummary::default();
    let sendable: Vec<OutgoingPerson> = people
        .into_iter()
        .filter_map(|(email, person)| {
            if email.is_none() {
                summary.no_email += 1;
                None
            } else {
                Some(person)
            }
        })
        .collect();
    if sendable.is_empty() {
        return Ok(summary);
    }

    let http = client()?;
    for (index, batch) in sendable.chunks(BATCH_SIZE).enumerate() {
        let response = http
            .post(format!("{}{API_PATH}", connection.url))
            .bearer_auth(&key)
            .json(&serde_json::json!({ "people": batch }))
            .send()
            .await
            .map_err(|e| {
                format!(
                    "Couldn't reach the CRM. Check your internet connection and try again. ({e})"
                )
            })?;
        if !response.status().is_success() {
            let reason = explain(response).await;
            return Err(if index == 0 {
                reason
            } else {
                format!(
                    "{reason} The first {} people were sent; push again to send the rest.",
                    index * BATCH_SIZE
                )
            });
        }
        let body: PushResponse = response.json().await.map_err(|_| {
            "The CRM's answer wasn't readable. Check the people in the CRM, then try again."
                .to_string()
        })?;
        summary.created += body.counts.created;
        summary.updated += body.counts.updated;
        summary.unchanged += body.counts.unchanged;
        summary.archived += body.counts.archived;
        summary.invalid += body.counts.invalid;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::normalise_url;

    #[test]
    fn tidies_the_address() {
        assert_eq!(
            normalise_url("crm.example.com").unwrap(),
            "https://crm.example.com"
        );
        assert_eq!(
            normalise_url(" https://crm.example.com/admin/ ").unwrap(),
            "https://crm.example.com"
        );
        assert_eq!(
            normalise_url("http://localhost:3000").unwrap(),
            "http://localhost:3000"
        );
        assert!(normalise_url("http://crm.example.com").is_err());
        assert!(normalise_url("").is_err());
        assert!(normalise_url("nonsense").is_err());
    }
}
