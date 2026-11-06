//! Standard uninstaller execution.
//!
//! Validates registered uninstall commands, rejects shell wrappers and arbitrary
//! script invocations, tokenizes arguments safely, and executes vendor uninstallers
//! with timeout, exit code tracking (including restart requirement 3010), and cancellation.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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
pub struct UninstallExecutionResult {
    pub app_id: String,
    pub exit_code: Option<i32>,
    pub status: String,
    pub restart_required: bool,
    pub message: String,
    pub elapsed_ms: u64,
}

fn is_forbidden_shell(name: &str) -> bool {
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

    let tokens = tokenize_command(trimmed);
    if tokens.is_empty() {
        return ParsedUninstallCommand {
            executable: String::new(),
            arguments: Vec::new(),
            is_msi: false,
            is_valid: false,
            rejection_reason: Some("Could not extract executable from command string.".into()),
        };
    }

    let raw_exe = &tokens[0];
    let mut args = tokens[1..].to_vec();

    // Check for MSI uninstaller
    let exe_name = Path::new(raw_exe)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(raw_exe)
        .to_ascii_lowercase();

    if exe_name == "msiexec.exe" || exe_name == "msiexec" {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        let system_msiexec = format!("{windir}\\System32\\msiexec.exe");

        // Normalize /I (install) or /X (uninstall) to /X
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

    // Verify executable exists
    let exe_path = PathBuf::from(raw_exe);
    if !exe_path.exists() {
        // Try looking up with .exe suffix
        let with_exe = exe_path.with_extension("exe");
        if with_exe.exists() {
            return ParsedUninstallCommand {
                executable: with_exe.to_string_lossy().to_string(),
                arguments: args,
                is_msi: false,
                is_valid: true,
                rejection_reason: None,
            };
        }
        return ParsedUninstallCommand {
            executable: raw_exe.clone(),
            arguments: args,
            is_msi: false,
            is_valid: false,
            rejection_reason: Some(format!("Uninstaller executable not found: {raw_exe}")),
        };
    }

    // Clean up arguments: remove any dangerous pipe or redirection syntax
    args.retain(|arg| !matches!(arg.as_str(), "|" | ">" | ">>" | "<" | "&" | "&&"));

    ParsedUninstallCommand {
        executable: exe_path.to_string_lossy().to_string(),
        arguments: args,
        is_msi: false,
        is_valid: true,
        rejection_reason: None,
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
                // Check if followed by quote
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
    let mut child = Command::new(&cmd.executable)
        .args(&cmd.arguments)
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
                let restart_required = code == 3010;
                let status_str = match code {
                    0 => "completed",
                    3010 => "restart_required",
                    1602 => "cancelled",
                    _ => "failed",
                };
                let message = match code {
                    0 => "Uninstaller completed successfully.".to_string(),
                    3010 => "Uninstaller finished; a system restart is required to complete removal.".to_string(),
                    1602 => "Uninstallation was cancelled by the user in the installer wizard.".to_string(),
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
}
