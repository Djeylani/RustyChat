use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncReadExt};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

pub async fn execute_code(language: &str, content: &str) -> ExecutionResult {
    let (cmd, args) = match language.to_lowercase().as_str() {
        "python" | "py" => ("python", vec!["-c", content]),
        "javascript" | "js" => ("node", vec!["-e", content]),
        "bash" | "sh" => ("sh", vec!["-c", content]),
        "powershell" | "ps1" => ("powershell", vec!["-Command", content]),
        _ => return ExecutionResult {
            stdout: "".to_string(),
            stderr: format!("Unsupported language for execution: {}", language),
            exit_code: Some(1),
        },
    };

    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut result = ExecutionResult {
        stdout: "".to_string(),
        stderr: "".to_string(),
        exit_code: None,
    };

    match child {
        Ok(mut child) => {
            let mut stdout = child.stdout.take().unwrap();
            let mut stderr = child.stderr.take().unwrap();

            let mut stdout_str = String::new();
            let mut stderr_str = String::new();

            let (_res_stdout, _res_stderr, res_status) = tokio::join!(
                stdout.read_to_string(&mut stdout_str),
                stderr.read_to_string(&mut stderr_str),
                child.wait()
            );

            result.stdout = stdout_str;
            result.stderr = stderr_str;
            
            if let Ok(status) = res_status {
                result.exit_code = status.code();
            }
        }
        Err(e) => {
            result.stderr = format!("Failed to spawn process: {}", e);
            result.exit_code = Some(1);
        }
    }

    result
}
