//! Agent CLI health: locating the Claude Code / Codex CLIs, recognising failures the
//! user fixes by installing, updating or signing in, installing them in the
//! background, and opening a terminal window for the sign-in.

use crate::model_config::Provider;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

/// Installers download ~150 MB, so allow for slow connections
const INSTALL_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// Windows: spawn console programs without flashing a console window
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Windows: give a spawned program its own visible console window
#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CliIssueKind {
    /// CLI is not installed or not on PATH
    Missing,
    /// The CLI rejected the selected model, which usually means it's too old
    Outdated,
    /// CLI is installed but not signed in
    Login,
    /// Account usage limit reached; nothing to fix locally
    Limit,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliAction {
    Install,
    Update,
    Login,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliIssuePayload {
    pub provider: Provider,
    pub kind: CliIssueKind,
    pub message: String,
}

/// One line of installer output, sent as the `cli-install-output` event
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliInstallOutput {
    provider: Provider,
    line: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub found: bool,
    pub path: Option<String>,
    /// None when the sign-in state could not be determined
    pub logged_in: Option<bool>,
    /// Installed version, e.g. "2.1.307"; None when it couldn't be read
    pub version: Option<String>,
}

/// Windows: force-kill a process and its children. Agent CLIs installed through npm run
/// behind a `cmd.exe` shim, so killing only the direct child would orphan the agent.
#[cfg(windows)]
pub fn kill_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;

    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

pub fn cli_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
    }
}

fn env_var(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "CLAUDE_PATH",
        Provider::Codex => "CODEX_PATH",
    }
}

pub fn emit_cli_issue(app: &AppHandle, provider: Provider, kind: CliIssueKind, message: String) {
    let _ = app.emit(
        "cli-issue",
        CliIssuePayload {
            provider,
            kind,
            message,
        },
    );
}

/// Where the official installers (and npm, as a fallback) put each CLI. Checked before
/// PATH because an installer's PATH change never reaches an app that is already running.
fn install_locations(provider: Provider) -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();

    #[cfg(windows)]
    let locations = {
        let local_app_data = dirs::data_local_dir().unwrap_or_default();
        let npm_dir = dirs::data_dir().unwrap_or_default().join("npm");
        match provider {
            Provider::Claude => vec![
                home.join(".local").join("bin").join("claude.exe"),
                npm_dir.join("claude.cmd"),
            ],
            Provider::Codex => vec![
                local_app_data.join(r"Programs\OpenAI\Codex\bin\codex.exe"),
                npm_dir.join("codex.cmd"),
            ],
        }
    };
    #[cfg(not(windows))]
    let locations = match provider {
        Provider::Claude => vec![
            home.join(".local/bin/claude"),
            home.join(".claude/local/claude"),
        ],
        Provider::Codex => vec![home.join(".local/bin/codex")],
    };

    locations
}

/// Windows: installers save their folder to the user's PATH in the registry, which a
/// running app doesn't see. Merge the saved PATH into ours so lookups and child
/// processes (npm's `.cmd` shims need `node`) work without restarting the app.
#[cfg(windows)]
fn refresh_path() {
    use std::os::windows::process::CommandExt;

    let Ok(output) = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
             [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + \
             [Environment]::GetEnvironmentVariable('Path', 'User')",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return;
    };

    let saved = String::from_utf8_lossy(&output.stdout);
    let mut entries: Vec<String> = std::env::var("PATH")
        .unwrap_or_default()
        .split(';')
        .filter(|entry| !entry.is_empty())
        .map(String::from)
        .collect();
    for entry in saved.trim().split(';') {
        if !entry.is_empty() && !entries.iter().any(|e| e.eq_ignore_ascii_case(entry)) {
            entries.push(entry.to_string());
        }
    }
    std::env::set_var("PATH", entries.join(";"));
}

/// Locate an agent CLI: explicit env var override, then the official install
/// locations, then the user's login shell PATH (GUI apps on macOS don't inherit it),
/// then the process PATH (resolves `.exe`/`.cmd` on Windows).
pub fn find_cli(provider: Provider) -> Option<String> {
    let name = cli_name(provider);
    if let Ok(path) = std::env::var(env_var(provider)) {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    if let Some(path) = install_locations(provider)
        .into_iter()
        .find(|p| p.is_file())
    {
        eprintln!("[cli_tools] Found {} at: {}", name, path.display());
        // An npm shim runs `node`, which a Node.js installed after Closer started has
        // only put on the saved PATH
        #[cfg(windows)]
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd"))
        {
            refresh_path();
        }
        return Some(path.to_string_lossy().to_string());
    }

    // Only use -l (login), not -i (interactive) to avoid extra output
    #[cfg(unix)]
    for shell in &["/bin/zsh", "/bin/bash"] {
        if let Ok(output) = std::process::Command::new(shell)
            .args(["-lc", &format!("which {name}")])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "[cli_tools] {} -lc 'which {}': status={}, stdout='{}', stderr='{}'",
                shell, name, output.status, stdout, stderr
            );

            if output.status.success() && !stdout.is_empty() && Path::new(&stdout).exists() {
                eprintln!("[cli_tools] Found {} at: {}", name, stdout);
                return Some(stdout);
            }
        }
    }

    #[cfg(windows)]
    refresh_path();

    if let Ok(path) = which::which(name) {
        eprintln!("[cli_tools] Found {} at: {}", name, path.display());
        return Some(path.to_string_lossy().to_string());
    }

    eprintln!("[cli_tools] Could not find {} CLI", name);
    None
}

const OUTDATED_PATTERNS: &[&str] = &[
    "is not supported when using codex",
    "model metadata for",
    "please update",
    "upgrade to the latest",
    "requires a newer version",
    "issue with the selected model",
];

const LOGIN_PATTERNS: &[&str] = &[
    "not logged in",
    "please run /login",
    "invalid api key",
    "authentication_error",
    "run `codex login`",
    "sign in again",
    "token expired",
    "refresh token",
];

const LIMIT_PATTERNS: &[&str] = &[
    "usage limit",
    "rate limit",
    "limit reached",
    "quota exceeded",
];

/// Classify a failed job's error output. Only error lines are passed in, so page
/// content the agent read during research can't trigger a false match.
pub fn detect_issue(error_output: &str) -> Option<(CliIssueKind, String)> {
    let checks = [
        (CliIssueKind::Outdated, OUTDATED_PATTERNS),
        (CliIssueKind::Login, LOGIN_PATTERNS),
        (CliIssueKind::Limit, LIMIT_PATTERNS),
    ];
    for (kind, patterns) in checks {
        let matched = error_output.lines().find(|line| {
            let line = line.to_lowercase();
            patterns.iter().any(|pattern| line.contains(pattern))
        });
        if let Some(line) = matched {
            return Some((kind, line.trim().chars().take(400).collect()));
        }
    }
    None
}

/// Pull the lines worth classifying out of a job's output: all of stderr plus the
/// stdout events that report the run's own error. Tool results also carry
/// `"is_error":true` (a page that failed to load), so only the final result counts.
pub fn error_lines(stdout: &str, stderr: &str) -> String {
    let stdout_errors = stdout.lines().filter(|line| {
        line.contains("\"type\":\"error\"")
            || (line.contains("\"type\":\"result\"") && line.contains("\"is_error\":true"))
    });
    stderr
        .lines()
        .chain(stdout_errors)
        .collect::<Vec<_>>()
        .join("\n")
}

#[tauri::command]
pub async fn check_cli(provider: Provider) -> Result<CliStatus, String> {
    let Some(path) = tauri::async_runtime::spawn_blocking(move || find_cli(provider))
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(CliStatus {
            found: false,
            path: None,
            logged_in: None,
            version: None,
        });
    };

    let status_args: &[&str] = match provider {
        Provider::Claude => &["auth", "status"],
        Provider::Codex => &["login", "status"],
    };
    let mut command = tokio::process::Command::new(&path);
    command.args(status_args).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let logged_in = command.output().await.ok().map(|o| o.status.success());

    let mut command = tokio::process::Command::new(&path);
    command.arg("--version").stdin(std::process::Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let version = command
        .output()
        .await
        .ok()
        .and_then(|o| parse_version(&String::from_utf8_lossy(&o.stdout)));

    Ok(CliStatus {
        found: true,
        path: Some(path),
        logged_in,
        version,
    })
}

/// "2.1.307 (Claude Code)" or "codex-cli 0.160.0" -> the version number
fn parse_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|word| word.starts_with(|c: char| c.is_ascii_digit()) && word.contains('.'))
        .map(String::from)
}

/// Open a terminal window running the install / update / sign-in command, so the
/// user can watch progress and answer any prompts (e.g. the browser sign-in).
#[tauri::command]
pub async fn open_cli_terminal(provider: Provider, action: CliAction) -> Result<(), String> {
    let path = tauri::async_runtime::spawn_blocking(move || find_cli(provider))
        .await
        .map_err(|e| e.to_string())?;
    let script = terminal_script(provider, action, path.as_deref());
    eprintln!("[cli_tools] Opening terminal: {}", script);
    open_terminal(&script)
}

/// The official standalone installer. It downloads a native binary, so it needs no
/// Node.js/npm, and re-running it updates an existing install.
fn installer_script(provider: Provider) -> &'static str {
    #[cfg(windows)]
    return match provider {
        Provider::Claude => "irm https://claude.ai/install.ps1 | iex",
        Provider::Codex => "irm https://chatgpt.com/codex/install.ps1 | iex",
    };
    #[cfg(not(windows))]
    return match provider {
        Provider::Claude => "curl -fsSL https://claude.ai/install.sh | bash",
        Provider::Codex => "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
    };
}

fn installer_command(provider: Provider) -> tokio::process::Command {
    #[cfg(windows)]
    let mut command = {
        // Older Windows PowerShell defaults to TLS 1.0, which the download hosts reject
        let script = format!(
            "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
             [Net.ServicePointManager]::SecurityProtocol = \
             [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12; \
             {}",
            installer_script(provider)
        );
        let mut command = tokio::process::Command::new("powershell");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
            ])
            .arg(script)
            .creation_flags(CREATE_NO_WINDOW);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        // pipefail so a failed download isn't reported as a successful install
        let mut command = tokio::process::Command::new("bash");
        command.args([
            "-c",
            &format!("set -o pipefail; {}", installer_script(provider)),
        ]);
        command
    };

    command.env("CODEX_NON_INTERACTIVE", "1");
    command
}

/// Forward each output line to the frontend and keep the tail for error messages
async fn pump_lines(
    app: AppHandle,
    provider: Provider,
    stream: impl AsyncRead + Unpin,
    tail: Arc<Mutex<Vec<String>>>,
) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        eprintln!("[cli_tools] installer: {}", line);
        let _ = app.emit(
            "cli-install-output",
            CliInstallOutput {
                provider,
                line: line.clone(),
            },
        );
        let mut tail = tail.lock().unwrap();
        tail.push(line);
        if tail.len() > 20 {
            tail.remove(0);
        }
    }
}

/// Install or update a CLI in the background with its official installer, streaming
/// progress as `cli-install-output` events. Resolves with the installed CLI's path.
#[tauri::command]
pub async fn install_cli(app: AppHandle, provider: Provider) -> Result<String, String> {
    eprintln!("[cli_tools] Installing {}", cli_name(provider));
    let mut child = installer_command(provider)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not start the installer: {e}"))?;

    let tail = Arc::new(Mutex::new(Vec::new()));
    let stdout = child
        .stdout
        .take()
        .map(|stream| tokio::spawn(pump_lines(app.clone(), provider, stream, tail.clone())));
    let stderr = child
        .stderr
        .take()
        .map(|stream| tokio::spawn(pump_lines(app.clone(), provider, stream, tail.clone())));

    let status = match tokio::time::timeout(INSTALL_TIMEOUT, child.wait()).await {
        Ok(status) => status.map_err(|e| format!("Installer failed: {e}"))?,
        Err(_) => {
            let _ = child.kill().await;
            return Err(
                "The installer took too long. Check your internet connection and try again.".into(),
            );
        }
    };
    for task in [stdout, stderr].into_iter().flatten() {
        let _ = task.await;
    }

    if !status.success() {
        let output = tail.lock().unwrap().join("\n");
        return Err(format!("The installer failed ({status}).\n{output}"));
    }

    tauri::async_runtime::spawn_blocking(move || find_cli(provider))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "The installer finished, but the {} CLI still can't be found.",
                cli_name(provider)
            )
        })
}

fn terminal_script(provider: Provider, action: CliAction, path: Option<&str>) -> String {
    // Use the exact binary the app runs, so sign-in reaches the copy jobs use
    #[cfg(windows)]
    let exe = path
        .map(|p| format!("& '{}'", p.replace('\'', "''")))
        .unwrap_or_else(|| cli_name(provider).to_string());
    #[cfg(not(windows))]
    let exe = path
        .map(|p| format!("'{}'", p.replace('\'', "'\\''")))
        .unwrap_or_else(|| cli_name(provider).to_string());

    match (provider, action) {
        (_, CliAction::Install | CliAction::Update) => installer_script(provider).to_string(),
        (Provider::Claude, CliAction::Login) => format!("{exe} auth login"),
        (Provider::Codex, CliAction::Login) => format!("{exe} login"),
    }
}

#[cfg(windows)]
fn open_terminal(script: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    std::process::Command::new("powershell")
        .args([
            "-NoExit",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open PowerShell: {e}"))
}

#[cfg(target_os = "macos")]
fn open_terminal(script: &str) -> Result<(), String> {
    let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
    std::process::Command::new("osascript")
        .args([
            "-e",
            &format!("tell application \"Terminal\" to do script \"{escaped}\""),
            "-e",
            "tell application \"Terminal\" to activate",
        ])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open Terminal: {e}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_terminal(script: &str) -> Result<(), String> {
    let shell_command = format!("{script}; exec $SHELL");
    for terminal in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
        let args: Vec<&str> = if terminal == "gnome-terminal" {
            vec!["--", "sh", "-c", &shell_command]
        } else {
            vec!["-e", "sh", "-c", &shell_command]
        };
        if std::process::Command::new(terminal)
            .args(args)
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }
    Err(format!(
        "No terminal found. Run this command yourself: {script}"
    ))
}

#[cfg(test)]
mod tests {
    use super::{detect_issue, error_lines, parse_version, CliIssueKind};

    #[test]
    fn parses_cli_versions() {
        assert_eq!(
            parse_version("2.1.307 (Claude Code)\n").as_deref(),
            Some("2.1.307")
        );
        assert_eq!(
            parse_version("codex-cli 0.160.0").as_deref(),
            Some("0.160.0")
        );
        assert_eq!(parse_version("error: unknown flag"), None);
    }

    #[test]
    fn detects_outdated_codex_from_model_rejection() {
        let output = r#"{"type":"error","error":"The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT account."}"#;
        assert_eq!(detect_issue(output).unwrap().0, CliIssueKind::Outdated);
    }

    #[test]
    fn detects_login_and_limit_errors() {
        assert_eq!(
            detect_issue("Invalid API key · Please run /login")
                .unwrap()
                .0,
            CliIssueKind::Login
        );
        assert_eq!(
            detect_issue("You've hit your usage limit").unwrap().0,
            CliIssueKind::Limit
        );
        assert!(detect_issue("Job failed for some other reason").is_none());
    }

    #[test]
    fn ignores_research_content_that_is_not_an_error() {
        let stdout = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"Our API has a rate limit"}]}}
{"type":"result","is_error":false}"#;
        assert!(detect_issue(&error_lines(stdout, "")).is_none());
    }

    #[test]
    fn ignores_failed_tool_calls_but_reads_the_final_result() {
        let tool_error = r#"{"type":"user","message":{"content":[{"type":"tool_result","is_error":true,"content":"HTTP 429: rate limit exceeded"}]}}"#;
        assert!(detect_issue(&error_lines(tool_error, "")).is_none());

        let result_error =
            r#"{"type":"result","is_error":true,"result":"You've hit your usage limit"}"#;
        assert_eq!(
            detect_issue(&error_lines(result_error, "")).unwrap().0,
            CliIssueKind::Limit
        );
    }
}
