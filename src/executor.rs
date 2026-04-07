use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const MAX_CAPTURE_BYTES: usize = 48 * 1024;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output_truncated: bool,
    pub working_directory: String,
}

pub async fn execute_code(language: &str, content: &str, timeout_secs: u64) -> ExecutionResult {
    let sandbox_dir = create_sandbox_dir();
    let sandbox_display = sandbox_dir.display().to_string();

    let (cmd, script_name, args) = match language.to_lowercase().as_str() {
        "python" | "py" => ("python", "snippet.py", Vec::<String>::new()),
        "javascript" | "js" => ("node", "snippet.js", Vec::<String>::new()),
        "bash" | "sh" => ("sh", "snippet.sh", Vec::<String>::new()),
        "powershell" | "ps1" => (
            "powershell",
            "snippet.ps1",
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
            ],
        ),
        _ => {
            return ExecutionResult {
                stdout: String::new(),
                stderr: format!("Unsupported language for execution: {}", language),
                exit_code: Some(1),
                timed_out: false,
                output_truncated: false,
                working_directory: sandbox_display,
            }
        }
    };

    let script_path = sandbox_dir.join(script_name);
    if let Err(e) = std::fs::write(&script_path, content) {
        return ExecutionResult {
            stdout: String::new(),
            stderr: format!("Failed to write temporary script: {}", e),
            exit_code: Some(1),
            timed_out: false,
            output_truncated: false,
            working_directory: sandbox_display,
        };
    }

    let mut full_args = args;
    full_args.push(script_path.to_string_lossy().to_string());

    let child = Command::new(cmd)
        .args(&full_args)
        .current_dir(&sandbox_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut result = ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
        timed_out: false,
        output_truncated: false,
        working_directory: sandbox_display,
    };

    match child {
        Ok(mut child) => {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let stdout_task = tokio::spawn(async move {
                match stdout {
                    Some(stdout) => read_stream_limited(stdout, MAX_CAPTURE_BYTES).await,
                    None => (String::new(), false),
                }
            });
            let stderr_task = tokio::spawn(async move {
                match stderr {
                    Some(stderr) => read_stream_limited(stderr, MAX_CAPTURE_BYTES).await,
                    None => (String::new(), false),
                }
            });

            match tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs.max(1)),
                child.wait(),
            )
            .await
            {
                Ok(Ok(status)) => {
                    result.exit_code = status.code();
                }
                Ok(Err(e)) => {
                    result.stderr = format!("Failed while waiting for process: {}", e);
                    result.exit_code = Some(1);
                }
                Err(_) => {
                    result.timed_out = true;
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    result.exit_code = Some(124);
                }
            }

            let (stdout_str, stdout_truncated) = stdout_task.await.unwrap_or_default();
            let (stderr_str, stderr_truncated) = stderr_task.await.unwrap_or_default();
            result.stdout = stdout_str;
            result.output_truncated = stdout_truncated || stderr_truncated;

            if result.stderr.is_empty() {
                result.stderr = stderr_str;
            } else if !stderr_str.is_empty() {
                result.stderr.push('\n');
                result.stderr.push_str(&stderr_str);
            }

            if result.timed_out {
                if !result.stderr.is_empty() {
                    result.stderr.push('\n');
                }
                result.stderr.push_str(&format!(
                    "Execution timed out after {} seconds and was stopped.",
                    timeout_secs.max(1)
                ));
            } else if result.output_truncated {
                if !result.stderr.is_empty() {
                    result.stderr.push('\n');
                }
                result.stderr.push_str("Output was truncated to keep the app responsive.");
            }
        }
        Err(e) => {
            result.stderr = format!("Failed to spawn process: {}", e);
            result.exit_code = Some(1);
        }
    }

    let _ = std::fs::remove_dir_all(&sandbox_dir);
    result
}

fn create_sandbox_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rustychat-run-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

async fn read_stream_limited<R>(mut reader: R, limit: usize) -> (String, bool)
where
    R: AsyncReadExt + Unpin,
{
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut chunk = [0_u8; 4096];

    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(read) => {
                let remaining = limit.saturating_sub(bytes.len());
                if remaining > 0 {
                    let keep = remaining.min(read);
                    bytes.extend_from_slice(&chunk[..keep]);
                }
                if read > remaining {
                    truncated = true;
                }
            }
            Err(_) => {
                truncated = true;
                break;
            }
        }
    }

    (String::from_utf8_lossy(&bytes).to_string(), truncated)
}
