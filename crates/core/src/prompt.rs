//! Turning a diagram and its review comments into text for an LLM: a brief
//! that describes the diagram, and prompts that ask an agent to act on the
//! open comments, either by editing the diagram or by changing code.
use serde::{Deserialize, Serialize};

use crate::dsl;
use crate::model::*;

/// What the agent should do with the comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    /// Change the diagram (respond with ops or a new diagram).
    #[default]
    Diagram,
    /// Change the codebase the diagram describes.
    Code,
    /// Answer questions / give feedback, without changing anything.
    Discuss,
}

/// How the agent should express diagram changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reply {
    /// `{"ops": [...]}` — precise, keeps everything else untouched.
    #[default]
    Ops,
    /// The whole updated diagram in the DSL.
    Dsl,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptOptions {
    pub target: Target,
    pub reply: Reply,
    /// Only these comments (all open ones when empty).
    pub comments: Vec<String>,
    /// Also include resolved comments (as context).
    pub include_resolved: bool,
    /// Extra instructions appended to the prompt.
    pub instructions: Option<String>,
    /// Path of the diagram file, so code agents can update it.
    pub file: Option<String>,
    /// How much of the board the prompt shows.
    pub context: Context,
}

/// How much of the board a prompt about comments shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Context {
    /// The whole board when it is small, else the commented sections.
    #[default]
    Auto,
    /// What the comments are on, their arrows and the blocks at the other ends.
    Near,
    /// The sections (or swimlanes, charts, maps) the commented blocks are in.
    Section,
    /// The whole board.
    All,
}

/// Boards up to this many lines go into prompts whole (`Context::Auto`).
const SMALL_BOARD: usize = 60;

/// The part of the board a prompt shows: what the comments are on, and
/// with `Section`, the blocks around it. None: the whole board.
fn context_ids(doc: &Document, comments: &[&Comment], context: Context) -> Option<Vec<String>> {
    let context = match context {
        Context::Auto if dsl::print(doc).lines().count() <= SMALL_BOARD => return None,
        Context::Auto => Context::Section,
        Context::All => return None,
        c => c,
    };
    let mut sels: Vec<String> = Vec::new();
    for c in comments {
        // a comment on the canvas is about the whole board
        let on = c.on.as_deref()?;
        if let Some(e) = doc.edge(on) {
            sels.push(format!("near:{}", e.from));
            sels.push(format!("near:{}", e.to));
            continue;
        }
        sels.push(format!("near:{on}"));
        if context == Context::Section {
            // the block it sits in: a map, a lane's swimlanes, a section
            let n = doc.node(on);
            let mut root = n;
            while let Some(p) = root
                .and_then(|x| x.parent.as_deref())
                .and_then(|p| doc.node(p))
            {
                root = Some(p);
            }
            if let Some(r) =
                root.filter(|r| r.mindmap.is_some() || n.is_some_and(|n| n.parent.is_some()))
            {
                sels.push(format!("map:{}", r.id));
            }
            let mut g = n
                .and_then(|n| root.unwrap_or(n).group.as_deref())
                .or(doc.group(on).map(|g| g.id.as_str()));
            if let Some(lanes) = g
                .and_then(|g| doc.group(g))
                .and_then(|g| g.parent.as_deref())
                .filter(|p| doc.group(p).is_some_and(|p| p.lanes))
            {
                g = Some(lanes);
            }
            if let Some(g) = g {
                sels.push(format!("group:{g}"));
            }
        }
    }
    let sels: Vec<&str> = sels.iter().map(String::as_str).collect();
    let mut ids = crate::query::select(doc, &sels).ok()?;
    ids.extend(comments.iter().map(|c| c.id.clone()));
    Some(ids)
}

pub const OPS_REFERENCE: &str = r#"Operations (fields in [] are optional; ids are letters, digits, `_`, `-`, `.`):
- {"op":"add_node","id":"cache","label":"Redis","shape":"cylinder",["color","detail","group","link","tags","x","y","w","h","parent","index","mindmap"]}
- {"op":"update_node","id":"api",["label","shape","color","detail","group","link","tags","w","h","parent","index","mindmap","collapsed","start","days","after","milestone","done","owner"]}   ("" clears a field)
- {"op":"add_outline","parent":"risks","outline":"Fraud\n  Card testing\nBank delays"}   (mindmap topics, two spaces per level; without parent: a new mindmap)
- {"op":"add_edge","from":"api","to":"cache",["id","label","arrow":"end|start|both|none","line":"solid|dashed|dotted|thick","color"]}
- {"op":"update_edge","id":"api->cache",["from","to","label","arrow","line","color"]}
- {"op":"add_group","id":"backend","label":"Backend",["color","parent","members":["api","db"]]}
- {"op":"update_group","id":"backend",["label","color","parent"]}
- {"op":"rename","id":"api","to":"orders-api"}            (updates edges and comments)
- {"op":"remove","id":"cache"}                            (node, group, edge or comment; a node's edges go too; a mindmap topic takes its branch)
- {"op":"move","id":"api","x":120,"y":40}  {"op":"unpin"[, "id":"api"]}
- {"op":"add_comment","text":"…",["on":"api","author"]}  {"op":"reply","id":"c1","text":"…",["author"]}
- {"op":"resolve_comment","id":"c1",["resolution":"what was done"]}  {"op":"reopen_comment","id":"c1"}
- {"op":"set","key":"title|description|direction|layout|routing|theme","value":"…"}
- {"op":"merge","dsl":"cache: Redis [cylinder]\napi -> cache: reads\napi [red]"}   (statements in the diagram language, as if written at the end of the file: adds new elements; one written again by its id gets the attributes given. Cheapest for many changes)
- Table: {"op":"add_group","id":"opts","label":"Options","columns":["option","cost","notes"]}; its rows are add_node / update_node with "group":"opts","label":"Local" (the first cell),"cells":["$0","needs a GPU"]. Or merge: "table opts: Options {\n  option | cost | notes\n  Local | $0 | needs a GPU\n}"
- Chart: {"op":"add_group","id":"rev","label":"Revenue","chart":"bar|line|area|pie|donut|scatter",["stacked":true,"horizontal":true,"unit":"%","series":["EU","US"],"axis":"quarter"]}; its rows are add_node / update_node with "group":"rev","label":"Q1","values":[120,80] (one per series, null for none). Or simpler, merge: "chart rev: Revenue [bar] {\n  quarter | EU | US\n  Q1 | 120 | 80\n}"
- Risk matrix: {"op":"add_group","id":"risks","label":"Risks","risks":true,["levels":3|4|5]}; its risks are add_node / update_node with "group":"risks" and ["probability":1-5,"impact":1-5,"owner","detail" (the description)]
- Gantt: a chart is {"op":"add_group","id":"plan","label":"Plan","gantt":true,["start":"YYYY-MM-DD","scale":"day|week|month|quarter"]}; its tasks are add_node / update_node with "group":"plan" and ["start","days","after":["id"],"milestone":true,"done":60,"owner":"ana"] (after may name tasks in other charts)
Shapes: box, rounded, pill, circle, diamond, hexagon, cylinder, parallelogram, document, person, cloud, sticky, text.
Colours: gray, blue, green, yellow, orange, red, purple, pink, teal or #rrggbb.
Edge ids default to `from->to` (`from->to#2` for a second edge between the same nodes).
Mindmaps: a topic's `parent` is the topic it branches from (`index` = position among siblings); a centre has `mindmap`: right|left|both|down. Prefer add_outline for new topics: it costs the fewest tokens."#;

fn quote_block(text: &str) -> String {
    text.lines().map(|l| format!("   > {l}\n")).collect()
}

fn comment_md(doc: &Document, c: &Comment, n: usize) -> String {
    let target = match &c.on {
        Some(on) => format!("on {}", doc.describe(on)),
        None => "on the whole diagram".to_string(),
    };
    let by = c
        .author
        .as_ref()
        .map(|a| format!(" — @{a}"))
        .unwrap_or_default();
    let status = if c.status == Status::Resolved {
        " (resolved)"
    } else {
        ""
    };
    let mut s = format!(
        "{n}. **{}** {target}{by}{status}\n{}",
        c.id,
        quote_block(&c.text)
    );
    for r in &c.replies {
        let by = r
            .author
            .as_ref()
            .map(|a| format!("@{a}: "))
            .unwrap_or_default();
        s.push_str(&format!("   - {by}{}\n", r.text.replace('\n', " ")));
    }
    if let Some(res) = &c.resolution {
        s.push_str(&format!("   - resolution: {}\n", res.replace('\n', " ")));
    }
    s
}

/// The comments a prompt covers.
pub fn selected<'a>(doc: &'a Document, opts: &PromptOptions) -> Vec<&'a Comment> {
    doc.comments
        .iter()
        .filter(|c| {
            if opts.comments.is_empty() {
                c.status == Status::Open || opts.include_resolved
            } else {
                opts.comments.contains(&c.id)
            }
        })
        .collect()
}

thread_local! {
    /// The day prompts forecast gantt charts from (see [`selection_at`]).
    static TODAY: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

fn with_today<T>(today: Option<i64>, f: impl FnOnce() -> T) -> T {
    let before = TODAY.with(|t| t.replace(today));
    let out = f();
    TODAY.with(|t| t.set(before));
    out
}

/// [`selection`] as of `today` (days since 1970): gantt tasks also say when
/// they are forecast to end and whether they are late or behind.
pub fn selection_at(doc: &Document, ids: &[String], today: Option<i64>) -> String {
    with_today(today, || selection(doc, ids))
}

/// [`brief`] as of `today`, with gantt forecasts (see [`selection_at`]).
pub fn brief_at(doc: &Document, today: Option<i64>) -> String {
    with_today(today, || brief(doc))
}

/// A description of the diagram for pasting into a chat or giving an agent
/// context: title and counts, what the source does not spell out (gantt
/// dates, swimlane steps in order), then the DSL source with every id.
pub fn brief(doc: &Document) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "# {}\n\n",
        doc.title.as_deref().unwrap_or("Diagram")
    ));
    if let Some(d) = &doc.description {
        s.push_str(&format!("{d}\n\n"));
    }
    let open = doc
        .comments
        .iter()
        .filter(|c| c.status == Status::Open)
        .count();
    s.push_str(&format!(
        "{} nodes, {} connections, {} groups, {} open comments. Flows {}.\n\n",
        doc.nodes.len(),
        doc.edges.len(),
        doc.groups.len(),
        open,
        doc.settings.direction.as_str()
    ));
    // what the source does not spell out: gantt dates and the order of
    // swimlane steps; the source follows, with every id
    let computed: Vec<String> = doc
        .groups
        .iter()
        .filter(|g| g.gantt || g.lanes || g.risks)
        .map(|g| g.id.clone())
        .collect();
    if !computed.is_empty() {
        let md = selection_core(doc, &computed, true);
        let md = md.split_once("\n\n").map(|(_, r)| r).unwrap_or("");
        s.push_str(md.trim_end());
        s.push('\n');
    }
    if !s.ends_with("\n\n") {
        s.push('\n');
    }
    s.push_str(&format!(
        "## Source\n\n```diagram\n{}```\n",
        dsl::print(doc)
    ));
    s
}

/// A prompt asking an agent to act on review comments.
pub fn prompt(doc: &Document, opts: &PromptOptions) -> String {
    let comments = selected(doc, opts);
    let title = doc.title.as_deref().unwrap_or("Untitled diagram");
    let file = opts.file.as_deref();
    let mut s = String::new();
    match opts.target {
        Target::Diagram => {
            s.push_str(&format!(
                "# Update the diagram \"{title}\" from review comments\n\n"
            ));
            s.push_str("The diagram below is written in the `diagram` language (one statement per line: `id: Label [attrs]` declares a node, `a -> b: label` connects nodes, `group id: Label { … }` groups them, `comment` / `reply` lines are review threads). Reviewers left comments on it. Address each comment listed under *Comments to address*.\n\n");
        }
        Target::Code => {
            s.push_str(&format!(
                "# Implement the changes requested in the \"{title}\" diagram review\n\n"
            ));
            s.push_str("The diagram below describes part of this codebase (its architecture, a flow or a data model). Reviewers commented on it; each comment is a requested change, a question or a concern. Make the corresponding changes in the code.\n\n");
        }
        Target::Discuss => {
            s.push_str(&format!(
                "# Answer review comments on the diagram \"{title}\"\n\n"
            ));
            s.push_str("Reviewers left comments on the diagram below. Answer each one: explain, suggest alternatives and point out risks. Do not change anything yet.\n\n");
        }
    }
    if let Some(d) = &doc.description {
        s.push_str(&format!("About the diagram: {d}\n\n"));
    }
    s.push_str("## Diagram\n\n");
    if let Some(f) = file {
        s.push_str(&format!("File: `{f}`\n\n"));
    }
    match context_ids(doc, &comments, opts.context) {
        Some(ids) => {
            let part = crate::query::subset(doc, &ids);
            let shown = part.nodes.len() + part.edges.len() + part.groups.len();
            let all = doc.nodes.len() + doc.edges.len() + doc.groups.len();
            s.push_str(&format!("The part of the board the comments are about ({shown} of its {all} elements). The rest exists too: keep its ids and do not remove or recreate it.\n\n"));
            s.push_str(&format!("```diagram\n{}```\n\n", dsl::print(&part)));
        }
        None => s.push_str(&format!("```diagram\n{}```\n\n", dsl::print(doc))),
    }
    s.push_str(&format!("## Comments to address ({})\n\n", comments.len()));
    if comments.is_empty() {
        s.push_str("_No open comments._\n");
    }
    for c in &comments {
        let n = doc.comments.iter().position(|x| x.id == c.id).unwrap() + 1;
        s.push_str(&comment_md(doc, c, n));
    }
    s.push_str("\n## How to respond\n\n");
    match (opts.target, opts.reply) {
        (Target::Diagram, Reply::Ops) => {
            s.push_str("Reply with a single ```json code block containing the operations to apply, for example:\n\n```json\n{\"ops\": [\n  {\"op\": \"add_node\", \"id\": \"cache\", \"label\": \"Redis\", \"shape\": \"cylinder\"},\n  {\"op\": \"add_edge\", \"from\": \"api\", \"to\": \"cache\", \"label\": \"reads\"},\n  {\"op\": \"resolve_comment\", \"id\": \"c1\", \"resolution\": \"Added a read-through cache\"}\n]}\n```\n\n");
            s.push_str(OPS_REFERENCE);
            s.push_str("\n\nRules:\n- Change only what the comments ask for; keep existing ids.\n- Resolve every comment you addressed with `resolve_comment` and a one-sentence `resolution`.\n- If a comment is a question or you disagree, answer with `reply` (author \"agent\") and leave it open.\n- The batch is applied atomically, so it must be valid as a whole.\n");
        }
        (Target::Diagram, Reply::Dsl) => {
            s.push_str("Reply with the complete updated diagram in a single ```diagram code block. Keep existing ids. Mark every comment you addressed as resolved by adding `[resolved, resolution: what you did]` at the end of its `comment` line. For questions, add a `reply <id> by agent: …` line and leave the comment open.\n");
        }
        (Target::Code, _) => {
            s.push_str("1. Make the code changes. Follow the conventions of the codebase.\n2. For each comment, report what you did (by comment id), or why you did not change anything.\n");
            match file {
                Some(f) => s.push_str(&format!("3. If the design changed, update the diagram to match and resolve the addressed comments:\n   `diagram apply {f} ops.json` (operations as below), or edit `{f}` directly and add `[resolved, resolution: …]` to the comment lines.\n\n{OPS_REFERENCE}\n")),
                None => s.push_str("3. If the design changed, also return the diagram changes as operations in a ```json {\"ops\": [...]} block, including a `resolve_comment` for each comment you addressed.\n\n"),
            }
            if file.is_none() {
                s.push_str(OPS_REFERENCE);
                s.push('\n');
            }
        }
        (Target::Discuss, _) => {
            s.push_str("Answer each comment under its id. If you want to record the answers in the diagram, also return a ```json block of `reply` operations, e.g. {\"ops\": [{\"op\": \"reply\", \"id\": \"c1\", \"author\": \"agent\", \"text\": \"…\"}]}.\n");
        }
    }
    if let Some(extra) = opts
        .instructions
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        s.push_str(&format!(
            "\n## Additional instructions\n\n{}\n",
            extra.trim()
        ));
    }
    s
}

/// The selected items (`ids`: nodes, groups, arrows) as Markdown to paste
/// into a prompt. One mindmap topic is that topic and its branches (see
/// [`crate::mindmap::to_markdown`]). Otherwise selected sections, swimlanes and
/// gantt charts bring their contents, topics bring their branches; blocks are
/// bullets under their section's heading, followed by the arrows between them
/// and their open comments.
///
/// A `#id` link reads "label (see “target”)", and what it points at that is
/// not already in the prompt follows it as a section of its own, so a prompt
/// written as a mindmap can pull in a flow or a plan from the same board.
pub fn selection(doc: &Document, ids: &[String]) -> String {
    let mut out = selection_core(doc, ids, false);
    linked(doc, ids, &mut out);
    out
}

/// How many linked blocks one prompt pulls in, at most.
const MAX_LINKED: usize = 8;

/// Append the blocks that `#id` links in the selection point at, and the ones
/// those link to, each once.
fn linked(doc: &Document, ids: &[String], out: &mut String) {
    let mut done = covered(doc, ids);
    let mut queue = links_in(doc, &done);
    // the first block that links to each target, to say where it comes from
    let from = |t: &str, among: &[String]| {
        doc.nodes
            .iter()
            .find(|n| {
                among.contains(&n.id)
                    && n.link.as_deref().and_then(|l| l.strip_prefix('#')) == Some(t)
            })
            .map(|n| n.display().replace('\n', " "))
    };
    let mut added = 0;
    while !queue.is_empty() && added < MAX_LINKED {
        let t = queue.remove(0);
        if done.contains(&t) || (doc.node(&t).is_none() && doc.group(&t).is_none()) {
            continue;
        }
        let md = match doc.node(&t) {
            Some(n) if n.parent.is_none() && n.mindmap.is_none() => plain_md(doc, n),
            _ => selection_core(doc, std::slice::from_ref(&t), false),
        };
        out.push('\n');
        let by = from(&t, &done);
        for (k, line) in md.lines().enumerate() {
            if line.starts_with('#') {
                out.push('#');
            }
            out.push_str(line);
            if k == 0 {
                if let Some(by) = &by {
                    out.push_str(&format!(" — linked from “{by}”"));
                }
            }
            out.push('\n');
        }
        let cov = covered(doc, std::slice::from_ref(&t));
        queue.extend(
            links_in(doc, &cov)
                .into_iter()
                .filter(|l| !done.contains(l)),
        );
        done.extend(cov);
        added += 1;
    }
}

/// Everything a selection brings: groups with what is inside them, nodes,
/// and the branches of topics.
fn covered(doc: &Document, ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = ids.to_vec();
    loop {
        let more: Vec<String> = doc
            .groups
            .iter()
            .filter(|g| g.parent.as_ref().is_some_and(|p| out.contains(p)))
            .map(|g| g.id.clone())
            .chain(
                doc.nodes
                    .iter()
                    .filter(|n| {
                        n.group.as_ref().is_some_and(|g| out.contains(g))
                            || n.parent.as_ref().is_some_and(|p| out.contains(p))
                    })
                    .map(|n| n.id.clone()),
            )
            .filter(|id| !out.contains(id))
            .collect();
        if more.is_empty() {
            return out;
        }
        out.extend(more);
    }
}

/// The targets of `#id` links on these nodes, in document order.
fn links_in(doc: &Document, ids: &[String]) -> Vec<String> {
    doc.nodes
        .iter()
        .filter(|n| ids.contains(&n.id))
        .filter_map(|n| n.link.as_deref()?.strip_prefix('#'))
        .map(str::to_string)
        .collect()
}

/// A single block: its label, detail, where it is and its arrows.
fn plain_md(doc: &Document, n: &Node) -> String {
    let label = |id: &str| {
        doc.node(id)
            .map(|n| n.display().replace('\n', " "))
            .or_else(|| doc.group(id).map(|g| g.display().to_string()))
            .unwrap_or_else(|| id.to_string())
    };
    let mut s = format!("# {}\n", n.display().replace('\n', " "));
    if let Some(d) = &n.detail {
        s.push_str(&format!("\n{d}\n"));
    }
    if let Some(g) = n.group.as_deref().and_then(|g| doc.group(g)) {
        s.push_str(&format!("\nIn “{}”.\n", g.display()));
    }
    let mut arrows = String::new();
    for e in &doc.edges {
        let l = e
            .label
            .as_deref()
            .map(|l| format!(" ({l})"))
            .unwrap_or_default();
        if e.from == n.id {
            arrows.push_str(&format!("- → {}{l}\n", label(&e.to)));
        } else if e.to == n.id {
            arrows.push_str(&format!("- ← {}{l}\n", label(&e.from)));
        }
    }
    if !arrows.is_empty() {
        s.push_str(&format!("\n{arrows}"));
    }
    s
}

/// A label for a prompt: a web link stays a Markdown link, a `#id` link says
/// what it points at (which the prompt includes).
pub(crate) fn link_md(doc: &Document, label: &str, link: Option<&str>) -> String {
    match link {
        Some(l) if l.starts_with('#') => {
            let t = &l[1..];
            match doc
                .node(t)
                .map(|n| n.display().replace('\n', " "))
                .or_else(|| doc.group(t).map(|g| g.display().to_string()))
            {
                Some(name) => format!("{label} (see “{name}”)"),
                None => label.to_string(),
            }
        }
        Some(l) => format!("[{label}]({l})"),
        None => label.to_string(),
    }
}

/// [`selection`] without linked blocks. `titled` always heads it with the
/// board's title (otherwise one section selected is its own title).
fn selection_core(doc: &Document, ids: &[String], titled: bool) -> String {
    if let [one] = ids {
        if doc
            .node(one)
            .is_some_and(|n| n.parent.is_some() || n.mindmap.is_some())
        {
            return crate::mindmap::to_markdown(doc, one);
        }
    }
    // what is in: selected groups with everything inside them, selected nodes
    let mut groups: Vec<&str> = doc
        .groups
        .iter()
        .filter(|g| ids.contains(&g.id))
        .map(|g| g.id.as_str())
        .collect();
    loop {
        let more: Vec<&str> = doc
            .groups
            .iter()
            .filter(|g| !groups.contains(&g.id.as_str()))
            .filter(|g| g.parent.as_deref().is_some_and(|p| groups.contains(&p)))
            .map(|g| g.id.as_str())
            .collect();
        if more.is_empty() {
            break;
        }
        groups.extend(more);
    }
    let nodes: Vec<&Node> = doc
        .nodes
        .iter()
        .filter(|n| ids.contains(&n.id) || n.group.as_deref().is_some_and(|g| groups.contains(&g)))
        .collect();
    let has = |id: &str| nodes.iter().any(|n| n.id == id);
    let mut out = String::new();
    // one section selected: its title is the title
    let top: Vec<&Group> = doc
        .groups
        .iter()
        .filter(|g| groups.contains(&g.id.as_str()))
        .filter(|g| !g.parent.as_deref().is_some_and(|p| groups.contains(&p)))
        .collect();
    let solo = !titled
        && top.len() == 1
        && nodes
            .iter()
            .all(|n| n.group.is_some() && groups.contains(&n.group.as_deref().unwrap()));
    let mut seen: Vec<String> = Vec::new();
    // steps of swimlanes: their arrows are part of the steps
    let mut flowed: Vec<String> = Vec::new();
    // blocks not inside an included group
    let loose: Vec<&Node> = nodes
        .iter()
        .copied()
        .filter(|n| !n.group.as_deref().is_some_and(|g| groups.contains(&g)))
        .filter(|n| !n.parent.as_deref().is_some_and(|p| has(p)))
        .collect();
    let block = |n: &Node, out: &mut String, seen: &mut Vec<String>| {
        seen.push(n.id.clone());
        out.push_str(&format!("- {}\n", item(doc, n)));
        crate::mindmap::bullets(doc, &n.id, 1, out, seen);
    };
    for n in &loose {
        block(n, &mut out, &mut seen);
    }
    fn section<'a>(
        doc: &'a Document,
        g: &Group,
        level: usize,
        groups: &[&str],
        nodes: &[&'a Node],
        out: &mut String,
        seen: &mut Vec<String>,
        flowed: &mut Vec<String>,
        block: &dyn Fn(&Node, &mut String, &mut Vec<String>),
    ) {
        if !out.is_empty() {
            out.push('\n');
        }
        let kind = if g.table {
            " (table)".to_string()
        } else if let Some(c) = g.chart {
            let mut k = format!(" ({} chart", c.as_str());
            if g.stacked {
                k.push_str(", stacked");
            }
            if g.horizontal {
                k.push_str(", horizontal");
            }
            if let Some(u) = &g.unit {
                k.push_str(&format!(", values in {u}"));
            }
            k.push(')');
            k
        } else if g.risks {
            let n = crate::risks::levels(g);
            format!(" (risk matrix, probability × impact, {n}×{n})")
        } else if g.gantt {
            match g.start.as_deref() {
                Some(s) => format!(" (gantt chart from {s})"),
                None => " (gantt chart)".into(),
            }
        } else if g.lanes {
            " (swimlanes)".into()
        } else if g
            .parent
            .as_deref()
            .and_then(|p| doc.group(p))
            .is_some_and(|p| p.lanes)
        {
            " (lane)".into()
        } else {
            String::new()
        };
        out.push_str(&format!(
            "{} {}{kind}\n",
            "#".repeat(level.min(6)),
            g.display()
        ));
        let inside: Vec<&&Node> = nodes
            .iter()
            .filter(|n| n.group.as_deref() == Some(&g.id))
            .collect();
        if !inside.is_empty() {
            out.push('\n');
        }
        if g.gantt {
            let s = crate::gantt::schedule(doc, &g.id);
            let forecast = TODAY
                .with(|t| t.get())
                .map(|t| crate::gantt::track(doc, &g.id, t))
                .unwrap_or_default();
            for (i, start, days) in s.tasks {
                let n = &doc.nodes[i];
                let when = match s.origin {
                    Some(o) => {
                        let from = o + start.floor() as i64;
                        let to = from + (days.ceil() as i64 - 1).max(0);
                        if to > from {
                            format!(
                                "{} → {}, ",
                                crate::gantt::format_date(from),
                                crate::gantt::format_date(to)
                            )
                        } else {
                            format!("{}, ", crate::gantt::format_date(from))
                        }
                    }
                    None => String::new(),
                };
                let after: Vec<&str> = n
                    .after
                    .iter()
                    .map(|a| doc.node(a).map(|t| t.display()).unwrap_or(a))
                    .collect();
                let after = if after.is_empty() {
                    String::new()
                } else {
                    format!(", after {}", after.join(", "))
                };
                let mut more = String::new();
                match n.done {
                    Some(d) if d >= 100.0 => more.push_str(", done"),
                    Some(d) => more.push_str(&format!(", {}% done", d.round())),
                    None => {}
                }
                if let Some(o) = &n.owner {
                    more.push_str(&format!(", owner {o}"));
                }
                let length = if n.milestone {
                    "milestone".to_string()
                } else {
                    crate::gantt::format_days(days)
                };
                // as of today: the forecast, when it is not the plan
                if let Some(t) = forecast.iter().find(|t| t.id == n.id) {
                    if t.slip > 0 {
                        let what = if n.milestone {
                            "forecast"
                        } else {
                            "forecast end"
                        };
                        more.push_str(&format!(
                            ", {what} {} (+{}d, {})",
                            t.forecast_end, t.slip, t.status
                        ));
                    } else if matches!(t.status, "late" | "behind") {
                        more.push_str(&format!(", {}", t.status));
                    }
                }
                seen.push(n.id.clone());
                out.push_str(&format!(
                    "- {}: {when}{length}{after}{more}\n",
                    item(doc, n)
                ));
            }
        } else if g.chart.is_some() {
            chart_table(doc, g, out, seen);
        } else if g.table {
            data_table(doc, g, out, seen);
        } else if g.risks {
            // worst first
            let n = crate::risks::levels(g);
            let mut list = crate::risks::risks_of(doc, &g.id);
            let score = |x: &Node| {
                crate::risks::rating(x, n).map_or(0, |(p, i)| u16::from(p) * u16::from(i))
            };
            list.sort_by_key(|x| std::cmp::Reverse(score(x)));
            for x in list {
                seen.push(x.id.clone());
                let label = link_md(doc, &x.display().replace('\n', " "), x.link.as_deref());
                let mut line = format!("- {label}: {}", crate::risks::summary(x, n).to_lowercase());
                if let Some(o) = &x.owner {
                    line.push_str(&format!(", owner {o}"));
                }
                if let Some(d) = &x.detail {
                    line.push_str(&format!(" — {}", d.replace('\n', " ")));
                }
                out.push_str(&line);
                out.push('\n');
            }
        } else if g.lanes {
            out.push('\n');
            steps(doc, g, out, seen, flowed);
        } else {
            for n in inside {
                if !n
                    .parent
                    .as_deref()
                    .is_some_and(|p| nodes.iter().any(|m| m.id == p))
                {
                    block(n, out, seen);
                }
            }
        }
        for sub in doc
            .groups
            .iter()
            .filter(|s| s.parent.as_deref() == Some(&g.id))
        {
            if groups.contains(&sub.id.as_str()) && !g.lanes {
                section(doc, sub, level + 1, groups, nodes, out, seen, flowed, block);
            }
        }
    }
    for g in &top {
        section(
            doc,
            g,
            if solo { 1 } else { 2 },
            &groups,
            &nodes,
            &mut out,
            &mut seen,
            &mut flowed,
            &block,
        );
    }
    // everything listed, topics included
    let listed = |id: &str| seen.iter().any(|s| s == id) || groups.contains(&id);
    let label = |id: &str| {
        doc.node(id)
            .map(|n| n.display().replace('\n', " "))
            .or_else(|| doc.group(id).map(|g| g.display().to_string()))
            .unwrap_or_else(|| id.to_string())
    };
    let arrows: Vec<&Edge> = doc
        .edges
        .iter()
        .filter(|e| ids.contains(&e.id) || (listed(&e.from) && listed(&e.to)))
        .filter(|e| !(flowed.contains(&e.from) && flowed.contains(&e.to)))
        .filter(|e| {
            !doc.node(&e.from).is_some_and(|n| {
                n.group
                    .as_deref()
                    .and_then(|g| doc.group(g))
                    .is_some_and(|g| g.gantt)
                    && doc.node(&e.to).is_some_and(|m| m.group == n.group)
            })
        })
        .collect();
    if !arrows.is_empty() {
        out.push_str("\n## Connections\n\n");
        for e in arrows {
            let l = e
                .label
                .as_deref()
                .map(|l| format!(": {l}"))
                .unwrap_or_default();
            out.push_str(&format!("- {} → {}{l}\n", label(&e.from), label(&e.to)));
        }
    }
    let comments: Vec<&Comment> = doc
        .comments
        .iter()
        .filter(|c| c.status == Status::Open)
        .filter(|c| {
            c.on.as_deref()
                .is_some_and(|on| listed(on) || ids.iter().any(|i| i == on))
        })
        .collect();
    if !comments.is_empty() {
        out.push_str("\n## Comments\n\n");
        for c in comments {
            out.push_str(&format!(
                "- On {}: {}\n",
                label(c.on.as_deref().unwrap()),
                c.text.replace('\n', " ")
            ));
            for r in &c.replies {
                out.push_str(&format!("  - {}\n", r.text.replace('\n', " ")));
            }
        }
    }
    let out = out.trim_start_matches('\n').to_string();
    if solo {
        out
    } else {
        let title = doc.title.as_deref().unwrap_or("Selection");
        format!("# {title}\n\n{out}")
    }
}

/// A table as a Markdown table.
fn data_table(doc: &Document, g: &Group, out: &mut String, seen: &mut Vec<String>) {
    let rows = crate::tables::rows_of(doc, &g.id);
    let n = rows
        .iter()
        .map(|r| r.cells.len() + 1)
        .max()
        .unwrap_or(1)
        .max(g.columns.len())
        .max(1);
    let cell = |s: &str| s.replace('|', "\\|").replace('\n', "<br>");
    let head: Vec<String> = (0..n)
        .map(|k| cell(g.columns.get(k).map(|s| s.as_str()).unwrap_or("")))
        .collect();
    out.push_str(&format!(
        "| {} |\n|{}\n",
        head.join(" | "),
        "---|".repeat(n)
    ));
    for r in rows {
        seen.push(r.id.clone());
        let mut cells = vec![link_md(doc, &cell(r.display()), r.link.as_deref())];
        cells.extend((1..n).map(|k| cell(r.cells.get(k - 1).map(|s| s.as_str()).unwrap_or(""))));
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
}

/// A chart as a Markdown table: the header, one row per row (a pie's also
/// with each slice's share).
fn chart_table(doc: &Document, g: &Group, out: &mut String, seen: &mut Vec<String>) {
    let rows = crate::charts::rows_of(doc, &g.id);
    let n = crate::charts::series_count(doc, g);
    let pie = matches!(g.chart, Some(ChartKind::Pie | ChartKind::Donut));
    let total: f64 = rows
        .iter()
        .map(|r| r.values.first().copied().flatten().unwrap_or(0.0).max(0.0))
        .sum();
    let mut head = vec![g.axis.clone().unwrap_or_default()];
    head.extend((0..n).map(|k| {
        if g.series.is_empty() && n == 1 {
            "value".to_string()
        } else {
            crate::charts::series_name(g, k)
        }
    }));
    if pie {
        head.push("share".into());
    }
    let cell = |s: &str| s.replace('|', "/").replace('\n', " ");
    out.push_str(&format!(
        "| {} |\n|{}\n",
        head.iter().map(|h| cell(h)).collect::<Vec<_>>().join(" | "),
        "---|".repeat(head.len())
    ));
    for r in rows {
        seen.push(r.id.clone());
        let mut cells = vec![link_md(doc, &cell(r.display()), r.link.as_deref())];
        cells.extend((0..n).map(|k| dsl::fmt_value(r.values.get(k).copied().flatten())));
        if pie {
            let v = r.values.first().copied().flatten().unwrap_or(0.0).max(0.0);
            cells.push(if total > 0.0 {
                format!("{}%", (v / total * 100.0).round())
            } else {
                "-".into()
            });
        }
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
}

/// Swimlanes as numbered steps in the order the flow runs (as laid out), each
/// with its lane; where a step does not simply lead to the next one, its
/// arrows say which step comes next (`yes → 4. Confirm order`).
fn steps(
    doc: &Document,
    g: &Group,
    out: &mut String,
    seen: &mut Vec<String>,
    flowed: &mut Vec<String>,
) {
    let lanes: Vec<&Group> = doc
        .groups
        .iter()
        .filter(|l| l.parent.as_deref() == Some(&g.id))
        .collect();
    let lane_of = |n: &Node| lanes.iter().position(|l| n.group.as_deref() == Some(&l.id));
    let mut list: Vec<&Node> = doc
        .nodes
        .iter()
        .filter(|n| n.parent.is_none() && lane_of(n).is_some())
        .collect();
    let lay = crate::layout::layout(doc);
    let along = |n: &Node| {
        let Some(b) = lay.nodes.iter().find(|b| b.id == n.id) else {
            return 0.0;
        };
        let (cx, cy) = (b.rect.x + b.rect.w / 2.0, b.rect.y + b.rect.h / 2.0);
        match doc.settings.direction {
            Direction::Right => cx,
            Direction::Left => -cx,
            Direction::Down => cy,
            Direction::Up => -cy,
        }
        .round()
    };
    list.sort_by(|a, b| {
        along(a)
            .total_cmp(&along(b))
            .then(lane_of(a).cmp(&lane_of(b)))
    });
    if !lanes.is_empty() {
        let names: Vec<&str> = lanes.iter().map(|l| l.display()).collect();
        out.push_str(&format!("Lanes: {}.\n\n", names.join(", ")));
    }
    let at = |id: &str| list.iter().position(|n| n.id == id);
    for (k, n) in list.iter().enumerate() {
        seen.push(n.id.clone());
        flowed.push(n.id.clone());
        let lane = lane_of(n).map(|l| lanes[l].display()).unwrap_or_default();
        let kind = if n.shape == Shape::Diamond {
            ", decision"
        } else {
            ""
        };
        out.push_str(&format!("{}. {} — {lane}{kind}\n", k + 1, item(doc, n)));
        let outs: Vec<&Edge> = doc
            .edges
            .iter()
            .filter(|e| e.from == n.id && at(&e.to).is_some())
            .collect();
        let simple = outs.len() == 1 && outs[0].label.is_none() && at(&outs[0].to) == Some(k + 1);
        if simple {
            continue;
        }
        for e in outs {
            let j = at(&e.to).unwrap();
            let l = e
                .label
                .as_deref()
                .map(|l| format!("{} ", l.replace('\n', " ")))
                .unwrap_or_default();
            out.push_str(&format!(
                "   - {l}→ {}. {}\n",
                j + 1,
                list[j].display().replace('\n', " ")
            ));
        }
    }
}

/// A block as one Markdown line: label, then its detail and link.
fn item(doc: &Document, n: &Node) -> String {
    let mut s = link_md(doc, &n.display().replace('\n', " "), n.link.as_deref());
    if let Some(d) = &n.detail {
        s.push_str(&format!(" — {}", d.replace('\n', " ")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        dsl::parse("title: Shop\napi: Orders API\napi -> db\ncomment on api by deyan: Split payments out?\nreply c1 by ana: Maybe later\ncomment on api->db: Use a replica [resolved]").unwrap()
    }

    #[test]
    fn prompt_lists_open_comments_and_instructions() {
        let p = prompt(&doc(), &PromptOptions::default());
        assert!(
            p.contains("**c1** on node `api` (\"Orders API\") — @deyan"),
            "{p}"
        );
        assert!(p.contains("@ana: Maybe later"));
        assert!(!p.contains("**c2**"));
        assert!(p.contains("resolve_comment"));
        let p = prompt(
            &doc(),
            &PromptOptions {
                include_resolved: true,
                target: Target::Code,
                file: Some("docs/shop.diagram".into()),
                ..Default::default()
            },
        );
        assert!(p.contains("**c2** on edge `api` → `db` (resolved)"), "{p}");
        assert!(p.contains("diagram apply docs/shop.diagram"));
    }

    #[test]
    fn prompts_show_the_part_of_a_big_board_the_comments_are_about() {
        let mut src = String::from(
            "title: Big\ngroup back: Backend {\n  api: Orders API\n  db: Postgres\n}\napi -> db\n",
        );
        for i in 0..70 {
            src.push_str(&format!("n{i}: Box {i}\n"));
        }
        src.push_str("comment on db by ana: Which version?\n");
        let d = dsl::parse(&src).unwrap();
        let auto = prompt(&d, &PromptOptions::default());
        assert!(
            auto.contains("The part of the board the comments are about (4 of its 74 elements)"),
            "{auto}"
        );
        assert!(
            auto.contains("group back: Backend {\n  api: Orders API\n  db: Postgres\n}")
                && !auto.contains("n42"),
            "{auto}"
        );
        let all = prompt(
            &d,
            &PromptOptions {
                context: Context::All,
                ..Default::default()
            },
        );
        assert!(all.contains("n42: Box 42") && !all.contains("The part of the board"));
        // a small board goes in whole
        assert!(!prompt(&doc(), &PromptOptions::default()).contains("The part of the board"));
    }

    #[test]
    fn selections_become_prompts() {
        let d = dsl::parse("title: Shop\ngroup back: Backend {\n  api: Orders API\n  db: Database [detail: Postgres]\n}\nweb: Web app\nweb -> api: places orders\napi -> db\ncomment on db: Which version?\ngantt g: Plan [start: 2026-10-12] {\n  Design [5d]\n  Build [1w, after: design]\n}\nmindmap m: Goals {\n  Fast\n    Under 1s\n}").unwrap();
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            selection(&d, &ids(&["web", "back"])),
            "# Shop\n\n- Web app\n\n## Backend\n\n- Orders API\n- Database — Postgres\n\n## Connections\n\n- Web app → Orders API: places orders\n- Orders API → Database\n\n## Comments\n\n- On Database: Which version?\n"
        );
        assert_eq!(
            selection(&d, &ids(&["back"])),
            "# Backend\n\n- Orders API\n- Database — Postgres\n\n## Connections\n\n- Orders API → Database\n\n## Comments\n\n- On Database: Which version?\n"
        );
        assert_eq!(
            selection(&d, &ids(&["g"])),
            "# Plan (gantt chart from 2026-10-12)\n\n- Design: 2026-10-12 → 2026-10-16, 5d\n- Build: 2026-10-17 → 2026-10-23, 1w, after Design\n"
        );
        assert_eq!(
            selection(&d, &ids(&["m"])),
            crate::mindmap::to_markdown(&d, "m")
        );
        assert_eq!(
            selection(&d, &ids(&["m", "web"])),
            "# Shop\n\n- Web app\n- Goals\n  - Fast\n    - Under 1s\n"
        );
    }

    #[test]
    fn brief_describes_everything() {
        let b = brief(&doc());
        assert!(b.starts_with("# Shop"));
        assert!(b.contains("```diagram\n"));
        assert!(
            b.contains("comment c1 on api by deyan: Split payments out?"),
            "{b}"
        );
        // gantt charts come with their dates
        let d =
            dsl::parse("title: P\ngantt g: Plan [start: 2026-10-12] {\n  Design [5d]\n}").unwrap();
        assert!(
            brief(&d).contains(
                "## Plan (gantt chart from 2026-10-12)\n\n- Design: 2026-10-12 → 2026-10-16, 5d\n"
            ),
            "{}",
            brief(&d)
        );
    }

    #[test]
    fn links_bring_what_they_point_at() {
        let d = dsl::parse("title: Board\nmindmap task: Task {\n  Context\n    The flow [link: #flow]\n    Docs [link: https://x.dev]\n}\ngroup flow: Flow {\n  a: Start [link: #note]\n  b: End\n}\nnote: Ask ops [detail: before launch]\na -> b\nnote -> b: informs").unwrap();
        let p = selection(&d, &["task".to_string()]);
        assert_eq!(
            p,
            "# Task\n\n## Context\n\n- The flow (see “Flow”)\n- [Docs](https://x.dev)\n\n## Flow — linked from “The flow”\n\n- Start (see “Ask ops”)\n- End\n\n### Connections\n\n- Start → End\n\n## Ask ops — linked from “Start”\n\nbefore launch\n\n- → End (informs)\n"
        );
        // already in the prompt: not repeated
        let p = selection(
            &d,
            &["task".to_string(), "flow".to_string(), "note".to_string()],
        );
        assert!(!p.contains("linked from"), "{p}");
    }

    #[test]
    fn swimlanes_are_numbered_steps() {
        let d = dsl::parse("title: T\nlanes o: Orders {\n  lane c: Customer {\n    place: Place order\n    fix: Fix details\n  }\n  lane s: Sales {\n    ok: Credit OK? [diamond]\n    confirm: Confirm\n  }\n}\nplace -> ok\nok -> confirm: yes\nok -> fix: no\nfix -> ok").unwrap();
        let p = selection(&d, &["o".to_string()]);
        assert!(p.starts_with("# Orders (swimlanes)\n\nLanes: Customer, Sales.\n\n1. Place order — Customer\n2. Credit OK? — Sales, decision\n"), "{p}");
        assert!(p.contains("   - yes → "), "{p}");
        assert!(p.contains("   - no → "), "{p}");
        assert!(!p.contains("Connections"), "{p}");
    }
}
