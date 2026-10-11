//! The AI harness: any command that reads a prompt on stdin and writes the
//! answer to stdout (`claude -p`, `codex exec -`, `llm -m gpt-4o-mini`,
//! `ollama run llama3`, …). Chosen with `--ai CMD` or `DIAGRAM_AI`; when
//! neither is set, Claude Code is used if it is installed.
use std::io::Write;
use std::process::{Command, Stdio};

/// The default asks a small, fast model: mindmap topics need few tokens.
const DEFAULT: &str = "claude -p --model haiku";

pub fn command(explicit: Option<&str>) -> Option<String> {
    if let Some(c) = explicit.filter(|c| !c.trim().is_empty()) {
        return Some(c.to_string());
    }
    if let Ok(c) = std::env::var("DIAGRAM_AI") {
        return Some(c).filter(|c| !c.trim().is_empty());
    }
    let found = Command::new("sh")
        .args(["-c", "command -v claude"])
        .output()
        .is_ok_and(|o| o.status.success());
    found.then(|| DEFAULT.to_string())
}

pub fn run(cmd: &str, prompt: &str) -> Result<String, String> {
    let mut child = Command::new("sh")
        .args(["-c", cmd])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run `{cmd}`: {e}"))?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(prompt.as_bytes())
        .map_err(|e| format!("`{cmd}`: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("`{cmd}`: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("`{cmd}` failed: {}", err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
