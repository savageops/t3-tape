use std::io::Write;
use std::process::{Command, Stdio};

use crate::exit::RedtapeError;

pub fn post(command: &str, body: &str) -> Result<String, RedtapeError> {
    let mut process = if cfg!(windows) {
        let mut child = Command::new("cmd");
        child.arg("/C").arg(command);
        child
    } else {
        let mut child = Command::new("sh");
        child.arg("-lc").arg(command);
        child
    };

    let mut child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(body.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RedtapeError::Agent(if stderr.is_empty() {
            format!("exec agent failed with {}", output.status)
        } else {
            stderr
        }));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
