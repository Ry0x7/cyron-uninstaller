//! Standard uninstaller execution.
//!
//! Validates registered uninstall commands, rejects shell wrappers and arbitrary
//! script invocations, tokenizes arguments safely, and executes vendor uninstallers
//! with timeout, exit code tracking (including restart requirement 3010), and cancellation.
//! Destruction commands require backend-generated, expiring, single-use plans.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedUninstallCommand {
    pub executable: String,
    pub arguments: Vec<String>,
    pub is_msi: bool,
    pub is_valid: bool,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredUninstallPlan {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub raw_command: String,
    pub command: ParsedUninstallCommand,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPlan {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub executable: String,
    pub arguments: Vec<String>,
    pub is_msi: bool,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUninstallItemPlan {
    pub app_id: String,
    pub app_name: String,
    pub raw_command: String,
    pub executable: String,
    pub arguments: Vec<String>,
    pub is_msi: bool,
    pub is_valid: bool,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredBatchUninstallPlan {
    pub plan_id: String,
    pub items: Vec<BatchUninstallItemPlan>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUninstallPlan {
    pub plan_id: String,
    pub items: Vec<BatchUninstallItemPlan>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallExecutionResult {
    pub app_id: String,
    pub exit_code: Option<i32>,
    pub status: String,
    pub restart_required: bool,
    pub message: String,
    pub elapsed_ms: u64,
}

pub fn is_forbidden_shell(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "cmd.exe"
            | "pwsh.exe"
            | "wscript.exe"
            | "cscript.exe"
            | "bash.exe"
            | "sh.exe"
            | "reg.exe"
            | "regedit.exe"
            | "rundll32.exe"
            | "mshta.exe"
    ) || lower.starts_with("pwsh")
        || (lower.starts_with("power") && lower.ends_with(".exe"))
}

/// Parses and validates a raw uninstall command string.
pub fn parse_uninstall_command(raw_command: &str) -> ParsedUninstallCommand {
    let trimmed = raw_command.trim();
    if trimmed.is_empty() {
        return ParsedUninstallCommand {
            executable: String::new(),
            arguments: Vec::new(),
            is_msi: false,
            is_valid: false,
            rejection_reason: Some("No uninstall command recorded for this application.".into()),
        };
    }

    // Resolve executable and arguments, handling both quoted and unquoted paths with spaces.
    let (raw_exe, args) = extract_executable_and_args(trimmed);
    if raw_exe.is_empty() {
        return ParsedUninstallCommand {
            executable: String::new(),
            arguments: Vec::new(),
            is_msi: false,
            is_valid: false,
            rejection_reason: Some("Could not extract executable from command string.".into()),
        };
    }

    // Check for MSI uninstaller
    let exe_name = Path::new(&raw_exe)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&raw_exe)
        .to_ascii_lowercase();

    if exe_name == "msiexec.exe" || exe_name == "msiexec" {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        let system_msiexec = format!("{windir}\\System32\\msiexec.exe");

        // Normalize /I (install) or /X (uninstall) to /x
        let mut normalized_args = Vec::new();
        let mut has_uninstall_switch = false;
        for arg in args {
            if arg.eq_ignore_ascii_case("/i") {
                normalized_args.push("/x".into());
                has_uninstall_switch = true;
            } else if arg.starts_with("/i") || arg.starts_with("/I") {
                normalized_args.push(format!("/x{}", &arg[2..]));
                has_uninstall_switch = true;
            } else {
                if arg.eq_ignore_ascii_case("/x") || arg.starts_with("/x") || arg.starts_with("/X") {
                    has_uninstall_switch = true;
                }
                normalized_args.push(arg);
            }
        }
        if !has_uninstall_switch {
            normalized_args.insert(0, "/x".into());
        }

        // Clean up dangerous shell arguments
        normalized_args.retain(|arg| !matches!(arg.as_str(), "|" | ">" | ">>" | "<" | "&" | "&&" | ";" | "^"));

        return ParsedUninstallCommand {
            executable: system_msiexec,
            arguments: normalized_args,
            is_msi: true,
            is_valid: true,
            rejection_reason: None,
        };
    }

    // Refuse forbidden shell interpreters
    if is_forbidden_shell(&exe_name) {
        return ParsedUninstallCommand {
            executable: raw_exe.clone(),
            arguments: args,
            is_msi: false,
            is_valid: false,
            rejection_reason: Some(format!(
                "Executing shell wrapper {exe_name} is refused for security."
            )),
        };
    }

    // Verify executable exists and has .exe extension
    let exe_path = PathBuf::from(&raw_exe);
    let resolved_path = if exe_path.exists() {
        exe_path
    } else {
        let with_exe = exe_path.with_extension("exe");
        if with_exe.exists() {
            with_exe
        } else {
            return ParsedUninstallCommand {
                executable: raw_exe.clone(),
                arguments: args,
                is_msi: false,
                is_valid: false,
                rejection_reason: Some(format!("Uninstaller executable not found: {raw_exe}")),
            };
        }
    };

    let extension = resolved_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension != "exe" {
        return ParsedUninstallCommand {
            executable: resolved_path.to_string_lossy().to_string(),
            arguments: args,
            is_msi: false,
            is_valid: false,
            rejection_reason: Some(format!(
                "Non-executable uninstaller format (.{extension}) is refused."
            )),
        };
    }

    // Clean up arguments: remove any dangerous pipe, redirection, or chaining syntax
    let mut safe_args = args;
    safe_args.retain(|arg| !matches!(arg.as_str(), "|" | ">" | ">>" | "<" | "&" | "&&" | ";" | "^"));

    let canonical_exe = match std::fs::canonicalize(&resolved_path) {
        Ok(c) => c.to_string_lossy().to_string(),
        Err(_) => resolved_path.to_string_lossy().to_string(),
    };

    ParsedUninstallCommand {
        executable: canonical_exe,
        arguments: safe_args,
        is_msi: false,
        is_valid: true,
        rejection_reason: None,
    }
}

/// Extracts executable and arguments from a command string.
/// Handles quoted executables as well as unquoted paths with spaces where an existing file ends in .exe.
fn extract_executable_and_args(cmd: &str) -> (String, Vec<String>) {
    let trimmed = cmd.trim();
    if trimmed.starts_with('"') {
        let tokens = tokenize_command(trimmed);
        if tokens.is_empty() {
            (String::new(), Vec::new())
        } else {
            (tokens[0].clone(), tokens[1..].to_vec())
        }
    } else {
        // If unquoted, check if the first tokenized executable exists.
        let tokens = tokenize_command(trimmed);
        if tokens.is_empty() {
            return (String::new(), Vec::new());
        }
        let first = &tokens[0];
        if Path::new(first).exists() || Path::new(&format!("{first}.exe")).exists() {
            return (first.clone(), tokens[1..].to_vec());
        }

        // If not, search for the first occurrence of .exe in trimmed string
        let lower = trimmed.to_ascii_lowercase();
        if let Some(idx) = lower.find(".exe") {
            let exe_end = idx + 4;
            let candidate_exe = trimmed[..exe_end].trim();
            if Path::new(candidate_exe).exists() {
                let remainder = trimmed[exe_end..].trim();
                let remaining_args = if remainder.is_empty() {
                    Vec::new()
                } else {
                    tokenize_command(remainder)
                };
                return (candidate_exe.to_string(), remaining_args);
            }
        }

        (tokens[0].clone(), tokens[1..].to_vec())
    }
}

/// Tokenizes a Windows command line with quotes and whitespace.
pub fn tokenize_command(cmd: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
            }
            '\\' => {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    current.push('\\');
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Creates a single-use, backend-verified uninstall plan with a 10-minute expiration.
pub fn create_uninstall_plan(
    app: &crate::apps::InstalledApp,
    quiet: bool,
) -> Result<StoredUninstallPlan, String> {
    let raw_cmd = if quiet && app.quiet_uninstall_string.is_some() {
        app.quiet_uninstall_string.as_ref().unwrap()
    } else if app.uninstall_string.is_some() {
        app.uninstall_string.as_ref().unwrap()
    } else {
        return Err(format!("No registered uninstall command found for {}.", app.name));
    };

    let parsed = parse_uninstall_command(raw_cmd);
    if !parsed.is_valid {
        return Err(parsed.rejection_reason.unwrap_or_else(|| "Invalid uninstaller command.".into()));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let nonce = crate::leftovers::generate_crypto_nonce(6);
    let plan_id = format!("uninst-{}-{}-{}-{}", now, std::process::id(), app.id.replace(|c: char| !c.is_ascii_alphanumeric(), "_"), nonce);

    Ok(StoredUninstallPlan {
        plan_id,
        app_id: app.id.clone(),
        app_name: app.name.clone(),
        raw_command: raw_cmd.clone(),
        command: parsed,
        expires_at: now + 600, // 10 minutes TTL
    })
}

/// Converts a stored plan to an outward-facing plan without sensitive execution secrets.
pub fn to_public_plan(plan: &StoredUninstallPlan) -> UninstallPlan {
    UninstallPlan {
        plan_id: plan.plan_id.clone(),
        app_id: plan.app_id.clone(),
        app_name: plan.app_name.clone(),
        executable: plan.command.executable.clone(),
        arguments: plan.command.arguments.clone(),
        is_msi: plan.command.is_msi,
        expires_at: plan.expires_at,
    }
}

/// Creates a single-use batch confirmation plan with server-side validation and 10-minute expiration.
pub fn create_batch_uninstall_plan(
    selected_apps: &[crate::apps::InstalledApp],
    quiet: bool,
) -> Result<StoredBatchUninstallPlan, String> {
    if selected_apps.is_empty() {
        return Err("Select at least one application to uninstall.".into());
    }
    if selected_apps.len() > 32 {
        return Err("Select at most 32 applications for batch uninstall.".into());
    }

    let mut items = Vec::new();
    let mut any_valid = false;

    for app in selected_apps {
        let raw_cmd = if quiet && app.quiet_uninstall_string.is_some() {
            app.quiet_uninstall_string.as_ref().unwrap()
        } else if app.uninstall_string.is_some() {
            app.uninstall_string.as_ref().unwrap()
        } else {
            items.push(BatchUninstallItemPlan {
                app_id: app.id.clone(),
                app_name: app.name.clone(),
                raw_command: String::new(),
                executable: String::new(),
                arguments: Vec::new(),
                is_msi: false,
                is_valid: false,
                rejection_reason: Some("No registered uninstaller command found.".into()),
            });
            continue;
        };

        let parsed = parse_uninstall_command(raw_cmd);
        if parsed.is_valid {
            any_valid = true;
        }
        items.push(BatchUninstallItemPlan {
            app_id: app.id.clone(),
            app_name: app.name.clone(),
            raw_command: raw_cmd.clone(),
            executable: parsed.executable,
            arguments: parsed.arguments,
            is_msi: parsed.is_msi,
            is_valid: parsed.is_valid,
            rejection_reason: parsed.rejection_reason,
        });
    }

    if !any_valid {
        return Err("None of the selected applications have a valid uninstaller command.".into());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let nonce = crate::leftovers::generate_crypto_nonce(6);
    let plan_id = format!("batch-{}-{}-{}-{}", now, std::process::id(), items.len(), nonce);

    Ok(StoredBatchUninstallPlan {
        plan_id,
        items,
        expires_at: now + 600, // 10 minutes TTL
    })
}

/// Converts a stored batch plan to an outward-facing plan for frontend review.
pub fn to_public_batch_plan(stored: &StoredBatchUninstallPlan) -> BatchUninstallPlan {
    BatchUninstallPlan {
        plan_id: stored.plan_id.clone(),
        items: stored.items.clone(),
        expires_at: stored.expires_at,
    }
}

/// Executes a validated uninstaller process and waits for completion.
pub fn execute_uninstaller(
    app_id: &str,
    cmd: &ParsedUninstallCommand,
    cancel: &AtomicBool,
    timeout_secs: u64,
) -> Result<UninstallExecutionResult, String> {
    if !cmd.is_valid {
        return Err(cmd
            .rejection_reason
            .clone()
            .unwrap_or_else(|| "The uninstaller command is invalid.".into()));
    }

    let started = Instant::now();
    let working_dir = Path::new(&cmd.executable)
        .parent()
        .filter(|p| p.is_dir());

    let mut command = Command::new(&cmd.executable);
    command.args(&cmd.arguments);
    if let Some(wd) = working_dir {
        command.current_dir(wd);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to launch uninstaller: {error}"))?;

    let timeout = Duration::from_secs(timeout_secs.clamp(10, 1800));
    let poll_interval = Duration::from_millis(250);

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            return Ok(UninstallExecutionResult {
                app_id: app_id.to_string(),
                exit_code: None,
                status: "cancelled".into(),
                restart_required: false,
                message: "Uninstallation was cancelled by user.".into(),
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                let restart_required = code == 3010 || code == 1641;
                let status_str = match code {
                    0 | 3010 | 1641 => "completed",
                    1602 => "cancelled",
                    _ => "failed",
                };
                let message = match code {
                    0 => "Uninstaller completed successfully.".to_string(),
                    3010 => "Uninstaller finished; a system restart is required to complete removal.".to_string(),
                    1641 => "Uninstaller finished; a system restart has been initiated.".to_string(),
                    1602 => "Uninstallation was cancelled by the user in the installer wizard.".to_string(),
                    1603 => "Fatal error during uninstallation (error 1603). The uninstaller may require elevated privileges or has missing files.".to_string(),
                    1618 => "Another installation is already in progress. Please wait for it to complete.".to_string(),
                    1605 => "This action is only valid for products that are currently installed (error 1605).".to_string(),
                    other => format!("Uninstaller exited with code {other}."),
                };
                return Ok(UninstallExecutionResult {
                    app_id: app_id.to_string(),
                    exit_code: Some(code),
                    status: status_str.into(),
                    restart_required,
                    message,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                });
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(format!("Uninstaller timed out after {} seconds.", timeout.as_secs()));
                }
                std::thread::sleep(poll_interval);
            }
            Err(error) => {
                return Err(format!("Error while monitoring uninstaller process: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_paths_and_arguments() {
        let cmd = r#""C:\Program Files\Vendor\app.exe" /S /D="C:\Program Files\Vendor""#;
        let tokens = tokenize_command(cmd);
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], r#"C:\Program Files\Vendor\app.exe"#);
        assert_eq!(tokens[1], "/S");
        assert_eq!(tokens[2], r#"/D=C:\Program Files\Vendor"#);
    }

    #[test]
    fn normalizes_msi_uninstallers() {
        let parsed = parse_uninstall_command("msiexec.exe /I{12345678-ABCD-EF01-2345-6789ABCDEF01}");
        assert!(parsed.is_valid);
        assert!(parsed.is_msi);
        assert!(parsed.arguments.iter().any(|arg| arg.starts_with("/x")));
    }

    #[test]
    fn rejects_forbidden_shell_wrappers() {
        let parsed = parse_uninstall_command("cmd.exe /c echo dangerous");
        assert!(!parsed.is_valid);
        assert!(parsed.rejection_reason.unwrap().contains("refused"));

        let parsed_ps = parse_uninstall_command("powershell.exe -Command Remove-Item C:\\");
        assert!(!parsed_ps.is_valid);
    }

    #[test]
    fn rejects_empty_commands() {
        let parsed = parse_uninstall_command("   ");
        assert!(!parsed.is_valid);
    }

    #[test]
    fn strips_dangerous_chaining_characters() {
        let parsed = parse_uninstall_command("msiexec.exe /x {GUID} & calc.exe | notepad.exe");
        assert!(parsed.is_valid);
        assert!(!parsed.arguments.iter().any(|a| a == "&" || a == "|"));
    }
}
