//! Local HTTP server for the editor: serves the web app, reads and writes the
//! diagram file, and reports external changes (the editor polls for them).
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::assets;

pub const FIRST_PORT: u16 = 5591;

fn bind(port: Option<u16>) -> Result<(Server, u16), String> {
    if let Some(p) = port {
        return Server::http(("127.0.0.1", p))
            .map(|s| (s, p))
            .map_err(|e| format!("cannot listen on port {p}: {e}"));
    }
    for p in FIRST_PORT..FIRST_PORT + 100 {
        if let Ok(s) = Server::http(("127.0.0.1", p)) {
            return Ok((s, p));
        }
    }
    Err("no free port found".into())
}

fn mtime(p: &Path) -> u128 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == key).then(|| url_decode(v))
    })
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
                {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn respond_json(req: Request, status: u16, v: Value) {
    let h = Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap();
    let _ = req.respond(
        Response::from_string(v.to_string())
            .with_status_code(status)
            .with_header(h),
    );
}

pub fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = std::process::Command::new(cmd.0)
        .args(cmd.1)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

pub fn serve(
    file: PathBuf,
    port: Option<u16>,
    open: bool,
    ai: Option<String>,
) -> Result<(), String> {
    let (server, port) = bind(port)?;
    let url = format!("http://127.0.0.1:{port}/");
    let display = file.display().to_string();
    eprintln!("diagram: editing {display} at {url}  (Ctrl-C to stop)");
    match &ai {
        Some(c) => eprintln!("diagram: ✦ AI via `{c}`"),
        None => eprintln!(
            "diagram: no AI harness (use --ai CMD); ✦ actions will show the prompt to copy"
        ),
    }
    if open {
        open_browser(&url);
    }
    let author = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("USER").ok());
    for mut req in server.incoming_requests() {
        let url = req.url().to_string();
        let path = url.split('?').next().unwrap_or("/").to_string();
        match (req.method().clone(), path.as_str()) {
            (Method::Get, "/api/file") => {
                let source = std::fs::read_to_string(&file).unwrap_or_default();
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // a theme file the board names comes along
                let (doc, _) = diagram_core::dsl::parse_lenient(&source);
                let themes: serde_json::Map<String, serde_json::Value> =
                    crate::theme_file(&doc, &file.to_string_lossy())
                        .ok()
                        .flatten()
                        .map(|(n, src)| (n, serde_json::Value::String(src)))
                        .into_iter()
                        .collect();
                respond_json(
                    req,
                    200,
                    json!({ "path": display, "name": name, "source": source, "themes": themes, "mtime": mtime(&file).to_string(), "author": author, "format": crate::format_for(&display), "version": env!("CARGO_PKG_VERSION") }),
                );
            }
            (Method::Get, "/api/ai") => respond_json(
                req,
                200,
                json!({ "available": ai.is_some(), "command": ai }),
            ),
            (Method::Post, "/api/ai") => {
                let mut prompt = String::new();
                let _ = req.as_reader().read_to_string(&mut prompt);
                let Some(cmd) = ai.clone() else {
                    respond_json(
                        req,
                        200,
                        json!({ "ok": false, "error": "no AI harness: start with --ai CMD or set DIAGRAM_AI" }),
                    );
                    continue;
                };
                // models take seconds; keep serving saves and polls meanwhile
                std::thread::spawn(move || match crate::ai::run(&cmd, &prompt) {
                    Ok(text) => respond_json(req, 200, json!({ "ok": true, "text": text })),
                    Err(e) => respond_json(req, 200, json!({ "ok": false, "error": e })),
                });
            }
            (Method::Get, "/api/version") => {
                respond_json(req, 200, json!({ "mtime": mtime(&file).to_string() }))
            }
            (Method::Put, "/api/file") => {
                let mut body = String::new();
                if req.as_reader().read_to_string(&mut body).is_err() {
                    respond_json(
                        req,
                        400,
                        json!({ "ok": false, "error": "body is not UTF-8" }),
                    );
                    continue;
                }
                // DSL files keep the text as typed; JSON / Mermaid files are converted
                let text = match crate::format_for(&display) {
                    "dsl" if diagram_core::detect(&body) == diagram_core::Format::Dsl => Ok(body),
                    fmt => diagram_core::parse(&body)
                        .map_err(|e| {
                            e.iter()
                                .map(|d| d.to_string())
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                        .and_then(|d| diagram_core::export(&d, fmt)),
                };
                match text.and_then(|t| std::fs::write(&file, t).map_err(|e| e.to_string())) {
                    Ok(()) => respond_json(
                        req,
                        200,
                        json!({ "ok": true, "mtime": mtime(&file).to_string() }),
                    ),
                    Err(e) => respond_json(req, 500, json!({ "ok": false, "error": e })),
                }
            }
            (Method::Get, "/api/git") => {
                let reference = query_param(&url, "ref").unwrap_or_else(|| "HEAD".into());
                match crate::git_show_pub(&reference, &display) {
                    Ok(source) => respond_json(
                        req,
                        200,
                        json!({ "ok": true, "source": source, "ref": reference }),
                    ),
                    Err(e) => respond_json(req, 200, json!({ "ok": false, "error": e })),
                }
            }
            (Method::Get, p) => match assets::get(p) {
                Some((bytes, ct)) => {
                    let h = Header::from_bytes("Content-Type", ct).unwrap();
                    let nc = Header::from_bytes("Cache-Control", "no-cache").unwrap();
                    let _ = req.respond(
                        Response::from_data(bytes.into_owned())
                            .with_header(h)
                            .with_header(nc),
                    );
                }
                None => {
                    let _ = req.respond(Response::from_string("not found").with_status_code(404));
                }
            },
            _ => {
                let _ =
                    req.respond(Response::from_string("method not allowed").with_status_code(405));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_query() {
        assert_eq!(
            super::query_param("/api/git?ref=HEAD%7E1&x=1", "ref").as_deref(),
            Some("HEAD~1")
        );
    }
}
