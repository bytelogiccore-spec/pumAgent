use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

pub struct TerminalTool {
    work_dir: PathBuf,
}

impl TerminalTool {
    pub fn new(mut work_dir: PathBuf) -> Self {
        // Ensure "Work" directory exists relative to base_dir
        work_dir.push("Work");

        if !work_dir.exists() {
            let _ = fs::create_dir_all(&work_dir);
        }

        TerminalTool { work_dir }
    }

    pub fn execute(&self, cmd_string: &str) -> Result<String, String> {
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
