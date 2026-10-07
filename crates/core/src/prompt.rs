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
}

pub const OPS_REFERENCE: &str = r#"Operations (fields in [] are optional; ids are letters, digits, `_`, `-`, `.`):
- {"op":"add_node","id":"cache","label":"Redis","shape":"cylinder",["color","detail","group","link","tags","x","y","w","h"]}
- {"op":"update_node","id":"api",["label","shape","color","detail","group","link","tags","w","h"]}   ("" clears a field)
- {"op":"add_edge","from":"api","to":"cache",["id","label","arrow":"end|start|both|none","line":"solid|dashed|dotted|thick","color"]}
- {"op":"update_edge","id":"api->cache",["from","to","label","arrow","line","color"]}
- {"op":"add_group","id":"backend","label":"Backend",["color","parent","members":["api","db"]]}
- {"op":"update_group","id":"backend",["label","color","parent"]}
- {"op":"rename","id":"api","to":"orders-api"}            (updates edges and comments)
- {"op":"remove","id":"cache"}                            (node, group, edge or comment; a node's edges go too)
- {"op":"move","id":"api","x":120,"y":40}  {"op":"unpin"[, "id":"api"]}
- {"op":"add_comment","text":"…",["on":"api","author"]}  {"op":"reply","id":"c1","text":"…",["author"]}
- {"op":"resolve_comment","id":"c1",["resolution":"what was done"]}  {"op":"reopen_comment","id":"c1"}
- {"op":"set","key":"title|description|direction|layout|routing|theme","value":"…"}
Shapes: box, rounded, pill, circle, diamond, hexagon, cylinder, parallelogram, document, person, cloud, sticky, text.
Colours: gray, blue, green, yellow, orange, red, purple, pink, teal or #rrggbb.
Edge ids default to `from->to` (`from->to#2` for a second edge between the same nodes)."#;

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

/// A plain-language description of the diagram, for pasting into a chat or
/// giving an agent context. The DSL source is included verbatim.
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
    if !doc.groups.is_empty() {
        s.push_str("## Groups\n\n");
        for g in &doc.groups {
            let (nodes, groups) = doc.members(Some(&g.id));
            let members: Vec<String> = nodes
                .iter()
                .map(|n| format!("`{}`", n.id))
                .chain(groups.iter().map(|g| format!("`{}`", g.id)))
                .collect();
            let parent = g
                .parent
                .as_ref()
                .map(|p| format!(", inside `{p}`"))
                .unwrap_or_default();
            s.push_str(&format!(
                "- `{}` \"{}\"{parent}: {}\n",
                g.id,
                g.display(),
                if members.is_empty() {
                    "empty".into()
                } else {
                    members.join(", ")
                }
            ));
        }
        s.push('\n');
    }
    s.push_str("## Nodes\n\n");
    for n in &doc.nodes {
        let mut bits = vec![n.shape.as_str().to_string()];
        if let Some(d) = &n.detail {
            bits.push(d.replace('\n', " "));
        }
        if !n.tags.is_empty() {
            bits.push(format!("tags: {}", n.tags.join(", ")));
        }
        if let Some(l) = &n.link {
            bits.push(l.clone());
        }
        s.push_str(&format!(
            "- `{}` \"{}\" ({})\n",
            n.id,
            n.display().replace('\n', " "),
            bits.join("; ")
        ));
    }
    if !doc.edges.is_empty() {
        s.push_str("\n## Connections\n\n");
        for e in &doc.edges {
            let arrow = match e.arrow {
                Arrow::End => "→",
                Arrow::Start => "←",
                Arrow::Both => "↔",
                Arrow::None => "—",
            };
            let label = e
                .label
                .as_ref()
                .map(|l| format!(": \"{}\"", l.replace('\n', " ")))
                .unwrap_or_default();
            let style = if e.line == Line::Solid {
                String::new()
            } else {
                format!(" ({})", e.line.as_str())
            };
            s.push_str(&format!(
                "- `{}` {arrow} `{}`{label}{style}\n",
                e.from, e.to
            ));
        }
    }
    if !doc.comments.is_empty() {
        s.push_str("\n## Comments\n\n");
        for (i, c) in doc.comments.iter().enumerate() {
            s.push_str(&comment_md(doc, c, i + 1));
        }
    }
    s.push_str(&format!(
        "\n## Source\n\n```diagram\n{}```\n",
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
    s.push_str(&format!("```diagram\n{}```\n\n", dsl::print(doc)));
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
    fn brief_describes_everything() {
        let b = brief(&doc());
        assert!(b.starts_with("# Shop"));
        assert!(b.contains("- `api` → `db`"));
        assert!(b.contains("```diagram\n"));
    }
}
