//! The AI harness: any command that reads a prompt on stdin and writes the
//! answer to stdout. Usually the CLI of an AI subscription the person
//! already has, signed in: `--ai claude` (Claude Code), `codex` (Codex CLI,
//! ChatGPT plan), `gemini` (Gemini CLI); `claude:sonnet` picks a model.
//! Any other text is a command line. Chosen with `--ai` or `DIAGRAM_AI`;
//! when neither is set, the first of those CLIs that is installed.
use std::io::Write;
use std::process::{Command, Stdio};

/// The subscription CLIs, in the order they are looked for: (name, the
/// command that reads a prompt on stdin and prints the answer).
pub const PRESETS: &[(&str, &str)] = &[
    ("claude", "claude -p"),
    ("codex", "codex exec --skip-git-repo-check -"),
    ("gemini", "gemini"),
];

/// A preset's command (`claude`, `claude:sonnet`), or the text as a command.
pub fn resolve(spec: &str) -> String {
    let spec = spec.trim();
    let (name, model) = match spec.split_once(':') {
        Some((n, m)) if !n.contains(' ') && !m.contains(' ') => (n, Some(m)),
        _ => (spec, None),
    };
    match PRESETS.iter().find(|(n, _)| *n == name) {
        Some((_, cmd)) => match (name, model) {
            ("claude", Some(m)) => format!("claude -p --model {m}"),
            ("codex", Some(m)) => format!("codex exec --skip-git-repo-check -m {m} -"),
            ("gemini", Some(m)) => format!("gemini -m {m}"),
            _ => cmd.to_string(),
        },
        None => spec.to_string(),
    }
}

pub fn command(explicit: Option<&str>) -> Option<String> {
    if let Some(c) = explicit.filter(|c| !c.trim().is_empty()) {
        return Some(resolve(c));
    }
    if let Ok(c) = std::env::var("DIAGRAM_AI") {
        return Some(resolve(&c)).filter(|c| !c.trim().is_empty());
    }
    // the first subscription CLI that is installed
    PRESETS.iter().find_map(|(name, cmd)| {
        Command::new("sh")
            .args(["-c", &format!("command -v {name}")])
            .output()
            .is_ok_and(|o| o.status.success())
            .then(|| cmd.to_string())
    })
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
