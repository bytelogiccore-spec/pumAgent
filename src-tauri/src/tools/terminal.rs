use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use std::sync::Arc;

pub struct TerminalTool {
    work_dir: PathBuf,
    pub dry_run: bool,
    db: Option<Arc<dbx_core::Database>>,
}

impl TerminalTool {
    pub fn new(mut work_dir: PathBuf, db: Option<Arc<dbx_core::Database>>) -> Self {
        // Ensure "Work" directory exists relative to base_dir
        work_dir.push("Work");

        if !work_dir.exists() {
            let _ = fs::create_dir_all(&work_dir);
        }

        TerminalTool {
            work_dir,
            dry_run: false,
            db,
        }
    }

    pub fn analyze_risk(&self, cmd_string: &str) -> Option<String> {
        let lower = cmd_string.to_lowercase();
        let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();

        let mut dangerous_keywords: Vec<(String, String)> = if cfg!(target_os = "windows") {
            vec![
                ("format".into(), "disk format command".into()),
                ("diskpart".into(), "disk partition tool".into()),
                ("remove-item".into(), "deletion command".into()),
                ("rmdir".into(), "directory deletion".into()),
                ("del".into(), "file deletion".into()),
                ("rd/s/q".into(), "recursive forced delete".into()),
                ("stop-process".into(), "process termination".into()),
                ("vssadmin".into(), "shadow copy manipulation".into()),
                ("wevtutilcl".into(), "event log clear".into()),
                ("wmic".into(), "system management command".into()),
                ("invoke-webrequest".into(), "network download".into()),
                ("curl".into(), "network download".into()),
                ("wget".into(), "network download".into()),
            ]
        } else {
            vec![
                ("rm-rf/".into(), "root-level destructive delete".into()),
                ("mkfs".into(), "filesystem format".into()),
                ("ddif=".into(), "raw disk write command".into()),
                ("shutdown".into(), "system shutdown".into()),
                ("reboot".into(), "system reboot".into()),
                ("sudo".into(), "privilege escalation".into()),
                ("chmod-r".into(), "recursive permission change".into()),
                ("chown-r".into(), "recursive ownership change".into()),
                ("curl".into(), "network download".into()),
                ("wget".into(), "network download".into()),
            ]
        };

        if let Some(ref db) = self.db {
            if let Ok(Some(custom_blocklist_bytes)) = db.get("config", b"terminal_blocklist") {
                if let Ok(custom_blocklist) = String::from_utf8(custom_blocklist_bytes) {
                    if let Ok(filters) = serde_json::from_str::<Vec<String>>(&custom_blocklist) {
                        dangerous_keywords.extend(
                            filters.into_iter().map(|s| {
                                (s.to_lowercase(), "custom blocklist pattern".to_string())
                            }),
                        );
                    }
                }
            }
        }

        for (kw, reason) in dangerous_keywords {
            let key = kw.replace(' ', "");
            if lower.contains(&kw) || compact.contains(&key) {
                return Some(reason);
            }
        }
        None
    }

    pub fn is_dangerous(&self, cmd_string: &str) -> bool {
        self.analyze_risk(cmd_string).is_some()
    }

    pub fn execute(&self, cmd_string: &str) -> Result<String, String> {
        if self.is_dangerous(cmd_string) {
            return Err("Security Error: The command contains dangerous keywords blocked by the system sandbox.".to_string());
        }

        if self.dry_run {
            println!(
                "🔒 [DRY-RUN MODE] Command intercepted before OS shell launch: {}",
                cmd_string
            );
            return Ok("Command executed successfully in DRY-RUN mode.".to_string());
        }

        // We use powershell to run the command on Windows
        // The sandbox relies on setting the current_dir.
        // Note: For absolute security, docker is needed, but this prevents accidental operations.

        let mut cmd;
        if cfg!(target_os = "windows") {
            cmd = Command::new("powershell");
            cmd.arg("-Command").arg(cmd_string);
        } else {
            cmd = Command::new("sh");
            cmd.arg("-c").arg(cmd_string);
        }

        cmd.current_dir(&self.work_dir);

        #[cfg(target_os = "windows")]
        {
            let create_no_window = 0x08000000;
            cmd.creation_flags(create_no_window);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let mut out_str = String::from_utf8_lossy(&output.stdout).to_string();
        let err_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !err_str.trim().is_empty() {
            out_str.push_str("\n[STDERR]\n");
            out_str.push_str(&err_str);
        }

        if !output.status.success() {
            return Err(format!(
                "Command exited with status: {}\nOutput: {}",
                output.status, out_str
            ));
        }

        if out_str.trim().is_empty() {
            return Ok("Command executed successfully with no output.".to_string());
        }

        Ok(out_str)
    }

    pub fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> crate::agent::multi_agent::ToolResult {
        if action == "execute" {
            let cmd_string = args.get("command").and_then(|c| c.as_str()).unwrap_or("");
            let result = self.execute(cmd_string);
            crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: result.is_ok(),
                output: result.unwrap_or_else(|e| e),
            }
        } else {
            crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unsupported action for terminal".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_compact_obfuscated_delete_pattern() {
        let tool = TerminalTool::new(std::env::temp_dir(), None);
        let reason = tool.analyze_risk("r d   / s / q  c:\\");
        assert!(reason.is_some());
    }

    #[test]
    fn allows_simple_safe_command() {
        let tool = TerminalTool::new(std::env::temp_dir(), None);
        assert!(!tool.is_dangerous("echo hello"));
    }
}
