mod ai;
mod assets;
mod export;
mod png;
mod server;
mod skills;

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use diagram_core::model::{Status, Theme};
use diagram_core::ops::Op;
use diagram_core::prompt::{PromptOptions, Reply, Target};
use diagram_core::render::CommentMode;
use diagram_core::{Document, RenderOptions};

pub const HELP: &str = r#"diagram — lightweight diagrams as text, with review comments that become LLM prompts

USAGE
  diagram [FILE]                     open FILE in the editor (created if missing;
                                     no FILE starts a new untitled.diagram)
  diagram new FILE [--template flow|architecture|board]   start a diagram from a template

EXPORT
  diagram svg  FILE [-o out.svg]     static SVG
  diagram png  FILE [-o out.png]     PNG image (--scale N, default 2)
  diagram html FILE [-o out.html]    self-contained interactive page (viewer + WASM)
    render options: --dark --transparent --title --comments open|all|none
                    --today | --as-of YYYY-MM-DD   mark that day on gantt charts and show the forecast
                    --focus ID,ID   crop to these elements (look at one block)
                    --base FILE|GIT_REF   highlight what changed since then
  diagram convert FILE --to dsl|json|mermaid|brief [-o OUT]
                                     FILE may be DSL, JSON or Mermaid (detected)
  diagram brief FILE [--today|--as-of DATE]   Markdown description for LLM context
                                     (with a date: gantt forecasts, late and behind tasks)

MINDMAPS
  diagram outline FILE [ID] [--md]   a mindmap as an indented outline (--md: Markdown, ready as a prompt)
  diagram expand  FILE ID [--ask TEXT] [--count N] [--levels N] [--print]
                                     grow topic ID with AI (prints the prompt with --print or without a harness)
  diagram expand  FILE --new TOPIC [--ask TEXT]   a new mindmap about TOPIC
    AI harness: --ai CMD or $DIAGRAM_AI (any command reading the prompt on stdin;
                default `claude -p --model haiku` when Claude Code is installed)

REVIEW
  diagram comments FILE [--all] [--json]          list comment threads
  diagram comment  FILE [--on ID] [--by NAME] TEXT  add a comment
  diagram reply    FILE COMMENT_ID [--by NAME] TEXT
  diagram resolve  FILE COMMENT_ID [--note TEXT]
  diagram prompt   FILE [--target diagram|code|discuss] [--reply ops|dsl]
                   [--comment c1,c2] [--all] [--instructions TEXT]
                   [--context auto|near|section|all]   how much of the board to show
                   (auto: all of a small board, else the commented sections)
                                     turn open comments into a prompt for an agent
  diagram apply    FILE [PATCH|-|'{"op":…}'] [--dry-run]   apply ops JSON (a file, stdin, inline,
                                     or an LLM reply containing it)

PLANNING
  diagram schedule FILE [CHART] [--as-of DATE] [--json]   gantt tasks planned vs forecast as of
                                     today (or DATE): late, behind, pushed back, how many days

FOR AGENTS
  diagram query FILE SELECTOR...     just part of the board, as diagram language (--ids: ids only)
                                     ID · group:ID · chart:ID · map:ID · near:ID · label~TEXT
                                     kind:box|topic|task|milestone|sticky|decision|group|edge|comment
                                     owner:NAME · tag:NAME · color:NAME · comments:open
  diagram add   FILE [DSL|-] [--dry-run]   add statements (new elements; one written again by its
                                     id gets the attributes given: `api [red]`)

CHECK
  diagram check FILE...              validate (exit 1 on errors)
  diagram fmt   FILE... [--check]    rewrite in canonical form
  diagram diff  OLD NEW [--json]     what changed (NEW may be a git ref: diagram diff FILE HEAD~1)

MORE
  diagram spec [--json-schema]       the language reference (for people and LLMs)
  diagram embed [-o diagram.embed.js]   browser library bundle (JS + inlined WASM)
  diagram skills list | show NAME | install [--global] [--dir PATH]   agent skills
  diagram version

SERVER OPTIONS
  --port N  --no-open  --ai CMD

FILE may be `-` for stdin. Formats are chosen by extension: .diagram (DSL),
.json, .mmd (Mermaid). See https://github.com/dobrinov/diagram
"#;

const VALUE_FLAGS: &[&str] = &[
    "-o",
    "--out",
    "--to",
    "--scale",
    "--target",
    "--reply",
    "--comment",
    "--comments",
    "--instructions",
    "--context",
    "--on",
    "--by",
    "--note",
    "--base",
    "--port",
    "--dir",
    "--template",
    "--theme",
    "--ai",
    "--ask",
    "--count",
    "--levels",
    "--new",
    "--focus",
    "--as-of",
];

struct Args {
    cmd: String,
    pos: Vec<String>,
    vals: HashMap<String, String>,
    flags: HashSet<String>,
}

impl Args {
    fn parse(raw: Vec<String>) -> Result<Args, String> {
        let mut pos = Vec::new();
        let mut vals = HashMap::new();
        let mut flags = HashSet::new();
        let mut it = raw.into_iter();
        while let Some(a) = it.next() {
            if let Some((k, v)) = a.split_once('=').filter(|(k, _)| k.starts_with("--")) {
                vals.insert(k.to_string(), v.to_string());
            } else if VALUE_FLAGS.contains(&a.as_str()) {
                let v = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                vals.insert(if a == "-o" { "--out".into() } else { a }, v);
            } else if a.starts_with('-') && a != "-" {
                flags.insert(a);
            } else {
                pos.push(a);
            }
        }
        const COMMANDS: &[&str] = &[
            "new", "open", "svg", "png", "html", "convert", "brief", "comments", "comment",
            "reply", "resolve", "prompt", "apply", "check", "fmt", "diff", "spec", "embed",
            "skills", "version", "help", "outline", "expand", "query", "add", "schedule",
        ];
        let cmd = match pos.first() {
            Some(c) if COMMANDS.contains(&c.as_str()) => pos.remove(0),
            _ => "open".into(),
        };
        if vals.contains_key("--out") {
            vals.insert("-o".into(), vals["--out"].clone());
        }
        Ok(Args {
            cmd,
            pos,
            vals,
            flags,
        })
    }
    fn val(&self, k: &str) -> Option<&str> {
        self.vals.get(k).map(|s| s.as_str())
    }
    fn flag(&self, k: &str) -> bool {
        self.flags.contains(k)
    }
    fn file(&self, i: usize) -> Result<&str, String> {
        self.pos
            .get(i)
            .map(|s| s.as_str())
            .ok_or_else(|| format!("missing FILE (try `diagram {} FILE`)", self.cmd))
    }
}

/// The first free `untitled.diagram`, `untitled-2.diagram`, … in the current directory.
fn untitled() -> String {
    (1..)
        .map(|n| match n {
            1 => "untitled.diagram".to_string(),
            n => format!("untitled-{n}.diagram"),
        })
        .find(|f| !Path::new(f).exists())
        .unwrap()
}

/// The first mindmap's centre.
fn first_map(doc: &Document) -> Result<String, String> {
    let f = diagram_core::mindmap::Forest::new(doc);
    (0..doc.nodes.len())
        .find(|&i| f.root[i] == Some(i))
        .map(|i| doc.nodes[i].id.clone())
        .ok_or_else(|| "this diagram has no mindmap".to_string())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match Args::parse(raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("diagram: {e}");
            return ExitCode::from(2);
        }
    };
    if args.flag("-h") || args.flag("--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match run(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("diagram: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn repo_root() -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn read_text(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        return Ok(s);
    }
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

/// A file's contents at a git ref (`HEAD`, `main`, `a1b2c3`, …).
fn git_show(reference: &str, file: &str) -> Result<String, String> {
    let abs = std::fs::canonicalize(file).map_err(|e| format!("{file}: {e}"))?;
    let dir = abs.parent().unwrap_or(Path::new("."));
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["ls-files", "--full-name"])
        .arg(&abs)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if rel.is_empty() {
        return Err(format!("{file} is not tracked by git"));
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["show", &format!("{reference}:{rel}")])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git show {reference}:{rel}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn git_show_pub(reference: &str, file: &str) -> Result<String, String> {
    git_show(reference, file)
}

fn parse_doc(src: &str, what: &str) -> Result<Document, String> {
    diagram_core::parse(src).map_err(|errs| {
        format!(
            "{what}:\n{}",
            errs.iter()
                .map(|e| format!("  {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

fn load(path: &str) -> Result<Document, String> {
    parse_doc(&read_text(path)?, path)
}

/// A base version: another file, or the same file at a git ref.
fn load_base(spec: &str, file: &str) -> Result<Document, String> {
    if Path::new(spec).exists() {
        load(spec)
    } else {
        parse_doc(&git_show(spec, file)?, &format!("{file} at {spec}"))
    }
}

pub fn format_for(path: &str) -> &'static str {
    if path.ends_with(".json") {
        "json"
    } else if path.ends_with(".mmd") || path.ends_with(".mermaid") {
        "mermaid"
    } else {
        "dsl"
    }
}

fn save(path: &str, doc: &Document) -> Result<(), String> {
    let format = format_for(path);
    let mut text = diagram_core::export(doc, format)?;
    if format == "dsl" && path != "-" {
        // keep the file as it is written: rewrite only what changed
        if let Ok(src) = std::fs::read_to_string(path) {
            if let Ok(old) = diagram_core::dsl::parse(&src) {
                text = diagram_core::patch::patch(&src, &old, doc);
            }
        }
    }
    if path == "-" {
        print!("{text}");
        return Ok(());
    }
    std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))
}

fn write_out(out: Option<&str>, content: &[u8], what: &str) -> Result<(), String> {
    match out {
        Some(p) if p != "-" => {
            std::fs::write(p, content).map_err(|e| format!("{p}: {e}"))?;
            eprintln!("wrote {what} to {p} ({} KB)", content.len().div_ceil(1024));
            Ok(())
        }
        _ => {
            use std::io::Write;
            std::io::stdout()
                .write_all(content)
                .map_err(|e| e.to_string())
        }
    }
}

fn author(args: &Args) -> Option<String> {
    if let Some(b) = args.val("--by") {
        return Some(b.to_string());
    }
    let git = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    git.or_else(|| std::env::var("USER").ok())
}

/// `--today` (the date now, UTC) or `--as-of YYYY-MM-DD`: the day gantt
/// charts mark and forecast from (days since 1970).
fn today(args: &Args) -> Result<Option<i64>, String> {
    if let Some(d) = args.val("--as-of") {
        return diagram_core::gantt::parse_date(d)
            .map(Some)
            .ok_or_else(|| format!("--as-of: `{d}` is not a date (YYYY-MM-DD)"));
    }
    if args.flag("--today") {
        return Ok(Some(today_utc()));
    }
    Ok(None)
}

/// Today (UTC), in days since 1970.
fn today_utc() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86400) as i64
}

fn render_opts(args: &Args) -> Result<RenderOptions, String> {
    let mut o = RenderOptions::default();
    if args.flag("--dark") {
        o.theme = Some(Theme::Dark);
    }
    if let Some(t) = args.val("--theme") {
        o.theme = Some(Theme::parse(t).ok_or_else(|| format!("unknown theme {t}"))?);
    }
    o.transparent = args.flag("--transparent");
    o.today = today(args)?.map(diagram_core::gantt::format_date);
    if args.flag("--title") {
        o.title = Some(true);
    }
    if let Some(f) = args.val("--focus") {
        o.focus = f
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    o.comments = match args.val("--comments").unwrap_or("open") {
        "open" => CommentMode::Open,
        "all" => CommentMode::All,
        "none" => CommentMode::None,
        m => return Err(format!("--comments must be open, all or none (got {m})")),
    };
    Ok(o)
}

fn rendered(args: &Args, file: &str) -> Result<(Document, diagram_core::Rendered), String> {
    let doc = load(file)?;
    let opts = render_opts(args)?;
    let r = match args.val("--base") {
        Some(b) => diagram_core::render_diff(&load_base(b, file)?, &doc, &opts),
        None => diagram_core::render::render(&doc, &opts),
    };
    Ok((doc, r))
}

fn default_out(file: &str, ext: &str) -> Option<String> {
    if file == "-" {
        return None;
    }
    let p = Path::new(file);
    let stem = p.file_name()?.to_str()?.split('.').next()?.to_string();
    Some(
        p.with_file_name(format!("{stem}.{ext}"))
            .to_string_lossy()
            .into_owned(),
    )
}

pub const TEMPLATES: &[(&str, &str)] = &[
    ("flow", include_str!("../../../examples/onboarding.diagram")),
    (
        "architecture",
        include_str!("../../../examples/checkout.diagram"),
    ),
    ("board", include_str!("../../../examples/retro.diagram")),
];

fn run(args: Args) -> Result<ExitCode, String> {
    let ok = Ok(ExitCode::SUCCESS);
    match args.cmd.as_str() {
        "help" => {
            print!("{HELP}");
            ok
        }
        "version" => {
            println!("diagram {}", env!("CARGO_PKG_VERSION"));
            ok
        }
        "open" => {
            let file = args.pos.first().cloned().unwrap_or_else(untitled);
            if !Path::new(&file).exists() {
                std::fs::write(&file, "idea: Start here\nidea -> next: then\n")
                    .map_err(|e| format!("{file}: {e}"))?;
                eprintln!("created {file}");
            }
            let port = args
                .val("--port")
                .map(|p| p.parse::<u16>().map_err(|_| "--port must be a number"))
                .transpose()?;
            let ai = ai::command(args.val("--ai"));
            server::serve(PathBuf::from(file), port, !args.flag("--no-open"), ai)?;
            ok
        }
        "new" => {
            let file = args.file(0)?;
            if Path::new(file).exists() {
                return Err(format!("{file} already exists"));
            }
            let name = args.val("--template").unwrap_or("flow");
            let (_, body) = TEMPLATES
                .iter()
                .find(|(n, _)| *n == name)
                .ok_or_else(|| format!("unknown template {name} (flow, architecture, board)"))?;
            let doc = parse_doc(body, name)?;
            save(file, &doc)?;
            eprintln!("created {file} from the {name} template — open it with `diagram {file}`");
            ok
        }
        "svg" => {
            let file = args.file(0)?;
            let (_, r) = rendered(&args, file)?;
            write_out(args.val("-o"), r.svg.as_bytes(), "SVG")?;
            ok
        }
        "png" => {
            let file = args.file(0)?;
            let (_, r) = rendered(&args, file)?;
            let scale = args
                .val("--scale")
                .map(|s| s.parse::<f32>().map_err(|_| "--scale must be a number"))
                .transpose()?
                .unwrap_or(2.0);
            let png = png::render(&r.svg, scale)?;
            let out = args
                .val("-o")
                .map(String::from)
                .or_else(|| default_out(file, "png"));
            write_out(out.as_deref(), &png, "PNG")?;
            ok
        }
        "html" => {
            let file = args.file(0)?;
            let src = read_text(file)?;
            let doc = parse_doc(&src, file)?;
            let base = args.val("--base").map(|b| load_base(b, file)).transpose()?;
            let html = export::html(&doc, base.as_ref(), &render_opts(&args)?);
            let out = args
                .val("-o")
                .map(String::from)
                .or_else(|| default_out(file, "html"));
            write_out(out.as_deref(), html.as_bytes(), "HTML")?;
            ok
        }
        "convert" | "brief" => {
            let file = args.file(0)?;
            let doc = load(file)?;
            let to = if args.cmd == "brief" {
                "brief"
            } else {
                args.val("--to")
                    .ok_or("--to dsl|json|mermaid|brief is required")?
            };
            let text = match today(&args)? {
                Some(t) if to == "brief" => diagram_core::prompt::brief_at(&doc, Some(t)),
                _ => diagram_core::export(&doc, to)?,
            };
            write_out(args.val("-o"), text.as_bytes(), to)?;
            ok
        }
        "schedule" => {
            let file = args.file(0)?;
            let doc = load(file)?;
            let now = match today(&args)? {
                Some(t) => t,
                None => today_utc(),
            };
            let charts: Vec<_> = doc
                .groups
                .iter()
                .filter(|g| g.gantt && args.pos.get(1).is_none_or(|c| *c == g.id))
                .collect();
            if charts.is_empty() {
                return Err(match args.pos.get(1) {
                    Some(c) => format!("no gantt chart `{c}`"),
                    None => "no gantt charts".into(),
                });
            }
            let all: Vec<_> = charts
                .iter()
                .map(|g| (g, diagram_core::gantt::track(&doc, &g.id, now)))
                .collect();
            if args.flag("--json") {
                let v: Vec<_> = all
                    .iter()
                    .map(|(g, t)| serde_json::json!({"id": g.id, "label": g.display(), "tasks": t}))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "today": diagram_core::gantt::format_date(now),
                        "charts": v
                    }))
                    .unwrap()
                );
                return ok;
            }
            for (g, tasks) in all {
                println!(
                    "{} ({}), as of {}",
                    g.display(),
                    g.id,
                    diagram_core::gantt::format_date(now)
                );
                let w = tasks
                    .iter()
                    .filter_map(|t| doc.node(&t.id))
                    .map(|n| n.display().chars().count())
                    .max()
                    .unwrap_or(4)
                    .min(36);
                for t in tasks {
                    let name: String = doc
                        .node(&t.id)
                        .map_or(t.id.clone(), |n| n.display().chars().take(w).collect());
                    let when = if t.milestone || t.start == t.end {
                        t.start.clone()
                    } else {
                        format!("{} → {}", t.start, t.end)
                    };
                    let done = t.done.map(|d| format!("{d}%")).unwrap_or_default();
                    let fc = if t.slip > 0 {
                        format!("forecast {} (+{}d)", t.forecast_end, t.slip)
                    } else {
                        String::new()
                    };
                    println!(
                        "  {name:<w$}  {when:<23}  {done:>4}  {:<11}  {fc}",
                        t.status
                    );
                }
                println!();
            }
            ok
        }
        "comments" => {
            let file = args.file(0)?;
            let doc = load(file)?;
            let list: Vec<_> = doc
                .comments
                .iter()
                .filter(|c| args.flag("--all") || c.status == Status::Open)
                .collect();
            if args.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&list).unwrap());
                return ok;
            }
            if list.is_empty() {
                eprintln!(
                    "no {}comments",
                    if args.flag("--all") { "" } else { "open " }
                );
            }
            for c in list {
                let on =
                    c.on.as_deref()
                        .map(|o| doc.describe(o))
                        .unwrap_or_else(|| "canvas".into());
                let by = c
                    .author
                    .as_deref()
                    .map(|a| format!(" @{a}"))
                    .unwrap_or_default();
                let st = if c.status == Status::Resolved {
                    " [resolved]"
                } else {
                    ""
                };
                println!(
                    "{} on {on}{by}{st}\n    {}",
                    c.id,
                    c.text.replace('\n', "\n    ")
                );
                for r in &c.replies {
                    println!(
                        "    ↳ {}{}",
                        r.author
                            .as_deref()
                            .map(|a| format!("@{a}: "))
                            .unwrap_or_default(),
                        r.text
                    );
                }
                if let Some(res) = &c.resolution {
                    println!("    ✓ {res}");
                }
            }
            ok
        }
        "comment" | "reply" | "resolve" => {
            let file = args.file(0)?;
            let mut doc = load(file)?;
            let op = match args.cmd.as_str() {
                "comment" => {
                    let text = args.pos[1..].join(" ");
                    if text.trim().is_empty() {
                        return Err("usage: diagram comment FILE [--on ID] TEXT".into());
                    }
                    Op::AddComment {
                        id: None,
                        on: args.val("--on").map(String::from),
                        text,
                        author: author(&args),
                        x: None,
                        y: None,
                    }
                }
                "reply" => {
                    let id = args
                        .pos
                        .get(1)
                        .ok_or("usage: diagram reply FILE COMMENT_ID TEXT")?
                        .clone();
                    let text = args.pos[2..].join(" ");
                    Op::Reply {
                        id,
                        text,
                        author: author(&args),
                    }
                }
                _ => {
                    let id = args
                        .pos
                        .get(1)
                        .ok_or("usage: diagram resolve FILE COMMENT_ID [--note TEXT]")?
                        .clone();
                    Op::ResolveComment {
                        id,
                        resolution: args.val("--note").map(String::from),
                    }
                }
            };
            let r = diagram_core::ops::apply(&mut doc, &[op]).map_err(|e| e.message)?;
            save(file, &doc)?;
            if let Some(id) = r.created.first() {
                println!("{id}");
            }
            ok
        }
        "prompt" => {
            let file = args.file(0)?;
            let doc = load(file)?;
            let target = match args.val("--target").unwrap_or("diagram") {
                "diagram" => Target::Diagram,
                "code" => Target::Code,
                "discuss" => Target::Discuss,
                t => {
                    return Err(format!(
                        "--target must be diagram, code or discuss (got {t})"
                    ))
                }
            };
            let reply = match args.val("--reply").unwrap_or("ops") {
                "ops" => Reply::Ops,
                "dsl" => Reply::Dsl,
                r => return Err(format!("--reply must be ops or dsl (got {r})")),
            };
            let comments: Vec<String> = args
                .val("--comment")
                .map(|c| {
                    c.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            for c in &comments {
                if doc.comment(c).is_none() {
                    return Err(format!("no comment {c}"));
                }
            }
            let opts = PromptOptions {
                target,
                reply,
                comments,
                include_resolved: args.flag("--all"),
                instructions: args.val("--instructions").map(String::from),
                file: (file != "-").then(|| file.to_string()),
                context: match args.val("--context").unwrap_or("auto") {
                    "auto" => diagram_core::prompt::Context::Auto,
                    "near" => diagram_core::prompt::Context::Near,
                    "section" => diagram_core::prompt::Context::Section,
                    "all" => diagram_core::prompt::Context::All,
                    c => {
                        return Err(format!(
                            "--context must be auto, near, section or all (got {c})"
                        ))
                    }
                },
            };
            print!("{}", diagram_core::prompt::prompt(&doc, &opts));
            ok
        }
        "outline" => {
            let file = args.file(0)?;
            let doc = load(file)?;
            let id = match args.pos.get(1) {
                Some(id) => id.clone(),
                None => first_map(&doc)?,
            };
            if doc.node(&id).is_none() {
                return Err(format!("no node `{id}`"));
            }
            if args.flag("--md") {
                print!(
                    "{}",
                    diagram_core::prompt::selection_at(&doc, &[id], today(&args)?)
                );
            } else {
                print!("{}", diagram_core::mindmap::outline(&doc, &id, true));
            }
            ok
        }
        "expand" => {
            let file = args.file(0)?;
            let mut doc = load(file)?;
            let o = diagram_core::mindmap::ExpandOptions {
                instructions: args.val("--ask").map(String::from),
                count: args.val("--count").and_then(|c| c.parse().ok()),
                levels: args.val("--levels").and_then(|c| c.parse().ok()),
            };
            let (prompt, parent) = match args.val("--new") {
                Some(topic) => (diagram_core::mindmap::generate_prompt(topic, &o), None),
                None => {
                    let id = args
                        .pos
                        .get(1)
                        .ok_or("usage: diagram expand FILE ID (or --new TOPIC)")?;
                    (
                        diagram_core::mindmap::expand_prompt(&doc, id, &o)?,
                        Some(id.clone()),
                    )
                }
            };
            let cmd = ai::command(args.val("--ai"));
            let Some(cmd) = cmd.filter(|_| !args.flag("--print")) else {
                print!("{prompt}");
                if !args.flag("--print") {
                    eprintln!(
                        "no AI harness found: pass --ai CMD or set DIAGRAM_AI; the prompt is above"
                    );
                }
                return ok;
            };
            eprintln!("asking `{cmd}`…");
            let reply = ai::run(&cmd, &prompt)?;
            let outline: String = diagram_core::mindmap::parse_reply(&reply)
                .iter()
                .map(|l| format!("{}{}\n", "  ".repeat(l.depth), l.text))
                .collect();
            let r = diagram_core::ops::apply(
                &mut doc,
                &[Op::AddOutline {
                    parent,
                    outline: outline.clone(),
                    index: None,
                    x: None,
                    y: None,
                }],
            )
            .map_err(|e| e.to_string())?;
            save(file, &doc)?;
            print!("{outline}");
            eprintln!("added {} topics to {file}", r.created.len());
            ok
        }
        "apply" => {
            let file = args.file(0)?;
            let mut doc = load(file)?;
            let patch = match args.pos.get(1).map(|s| s.as_str()).unwrap_or("-") {
                // ops right on the command line
                inline if inline.trim_start().starts_with(['{', '[']) => inline.to_string(),
                path => read_text(path)?,
            };
            // a reply may also be a whole new diagram in a ```diagram block
            let ops = match diagram_core::ops::parse_ops(&patch) {
                Ok(ops) => ops,
                Err(e) => match patch.find("```diagram") {
                    Some(start) => {
                        let body = &patch[start + 10..];
                        let body = &body[..body.find("```").unwrap_or(body.len())];
                        let new = parse_doc(body, "the ```diagram block")?;
                        if args.flag("--dry-run") {
                            print!(
                                "{}",
                                diagram_core::diff::markdown(
                                    &diagram_core::diff::diff(&doc, &new),
                                    &doc,
                                    &new
                                )
                            );
                        } else {
                            save(file, &new)?;
                            eprintln!("replaced {file} with the diagram from the reply");
                        }
                        return ok;
                    }
                    None => return Err(e),
                },
            };
            let before = doc.clone();
            let r = diagram_core::ops::apply(&mut doc, &ops).map_err(|e| e.to_string())?;
            let d = diagram_core::diff::diff(&before, &doc);
            print!("{}", diagram_core::diff::markdown(&d, &before, &doc));
            if args.flag("--dry-run") {
                eprintln!("dry run: {} ops ok, {file} not changed", r.count);
            } else {
                save(file, &doc)?;
                eprintln!("applied {} ops to {file}", r.count);
            }
            ok
        }
        "query" => {
            let file = args.file(0)?;
            let doc = load(file)?;
            let sels: Vec<&str> = args.pos[1..].iter().map(|s| s.as_str()).collect();
            if sels.is_empty() {
                return Err("usage: diagram query FILE SELECTOR... (see `diagram help`)".into());
            }
            if args.flag("--ids") {
                for id in diagram_core::query::select(&doc, &sels)? {
                    println!("{id}");
                }
            } else {
                print!("{}", diagram_core::query::query(&doc, &sels)?);
            }
            ok
        }
        "add" => {
            let file = args.file(0)?;
            let mut doc = load(file)?;
            let dsl = match args.pos.get(1).map(|s| s.as_str()).unwrap_or("-") {
                "-" => read_text("-")?,
                text => text.to_string(),
            };
            let before = doc.clone();
            let r =
                diagram_core::ops::apply(&mut doc, &[Op::Merge { dsl }]).map_err(|e| e.message)?;
            print!(
                "{}",
                diagram_core::diff::markdown(
                    &diagram_core::diff::diff(&before, &doc),
                    &before,
                    &doc
                )
            );
            if args.flag("--dry-run") {
                eprintln!("dry run: {file} not changed");
            } else {
                save(file, &doc)?;
                eprintln!("added to {file} ({} new)", r.created.len());
            }
            ok
        }
        "check" => {
            let mut failed = false;
            if args.pos.is_empty() {
                return Err("usage: diagram check FILE...".into());
            }
            for f in &args.pos {
                let src = read_text(f)?;
                match diagram_core::parse(&src) {
                    Ok(doc) => {
                        let problems = doc.check();
                        if problems.is_empty() {
                            let open = doc
                                .comments
                                .iter()
                                .filter(|c| c.status == Status::Open)
                                .count();
                            eprintln!(
                                "{f}: ok ({} nodes, {} edges, {open} open comments)",
                                doc.nodes.len(),
                                doc.edges.len()
                            );
                        } else {
                            failed = true;
                            for p in problems {
                                eprintln!("{f}: {p}");
                            }
                        }
                    }
                    Err(errs) => {
                        failed = true;
                        for e in errs {
                            eprintln!("{f}:{}: {}", e.line, e.message);
                        }
                    }
                }
            }
            Ok(if failed {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        "fmt" => {
            let mut dirty = false;
            for f in &args.pos {
                let src = read_text(f)?;
                let doc = parse_doc(&src, f)?;
                let out = diagram_core::export(&doc, format_for(f))?;
                if out != src {
                    dirty = true;
                    if args.flag("--check") {
                        eprintln!("{f} is not formatted");
                    } else if f == "-" {
                        print!("{out}");
                    } else {
                        std::fs::write(f, out).map_err(|e| e.to_string())?;
                        eprintln!("formatted {f}");
                    }
                } else if f == "-" {
                    print!("{out}");
                }
            }
            Ok(if dirty && args.flag("--check") {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        "diff" => {
            let a = args.file(0)?;
            let b = args
                .pos
                .get(1)
                .ok_or("usage: diagram diff OLD NEW  or  diagram diff FILE GIT_REF")?;
            // `diagram diff FILE REF` compares the file at REF (old) with the working copy (new)
            let (old, new) = if Path::new(b).exists() {
                (load(a)?, load(b)?)
            } else {
                (load_base(b, a)?, load(a)?)
            };
            let d = diagram_core::diff::diff(&old, &new);
            if args.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&d).unwrap());
            } else {
                print!("{}", diagram_core::diff::markdown(&d, &old, &new));
            }
            ok
        }
        "spec" => {
            if args.flag("--json-schema") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&diagram_core::json_schema()).unwrap()
                );
            } else {
                print!("{}", diagram_core::spec());
            }
            ok
        }
        "embed" => {
            let out = args.val("-o").unwrap_or("diagram.embed.js");
            write_out(
                Some(out),
                export::embed_bundle().as_bytes(),
                "the embed bundle",
            )?;
            ok
        }
        "skills" => {
            skills::run(
                &args.pos,
                args.flag("--global"),
                args.val("--dir").map(PathBuf::from),
            )?;
            ok
        }
        c => Err(format!("unknown command {c}")),
    }
}
