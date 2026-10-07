//! Mermaid flowcharts in and out. LLMs write Mermaid fluently and GitHub
//! renders it, so it is both an import path and a sharing format.
use crate::model::*;

fn mermaid_id(id: &str) -> String {
    let s: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.chars().next().is_some_and(|c| c.is_ascii_digit())
        || matches!(s.as_str(), "end" | "graph" | "subgraph" | "flowchart")
    {
        format!("n_{s}")
    } else {
        s
    }
}

fn mermaid_text(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "#quot;").replace('\n', "<br/>"))
}

pub fn export(doc: &Document) -> String {
    let dir = match doc.settings.direction {
        Direction::Right => "LR",
        Direction::Down => "TB",
        Direction::Left => "RL",
        Direction::Up => "BT",
    };
    let mut out = String::new();
    if let Some(t) = &doc.title {
        out.push_str(&format!("---\ntitle: {}\n---\n", t.replace('\n', " ")));
    }
    out.push_str(&format!("flowchart {dir}\n"));
    fn node(n: &Node) -> String {
        let mut label = n.display().to_string();
        if let Some(d) = &n.detail {
            label.push('\n');
            label.push_str(d);
        }
        let t = mermaid_text(&label);
        let (open, close) = match n.shape {
            Shape::Rounded => ("(", ")"),
            Shape::Pill => ("([", "])"),
            Shape::Circle => ("((", "))"),
            Shape::Diamond => ("{", "}"),
            Shape::Hexagon => ("{{", "}}"),
            Shape::Cylinder => ("[(", ")]"),
            Shape::Parallelogram => ("[/", "/]"),
            _ => ("[", "]"),
        };
        format!("{}{open}{t}{close}", mermaid_id(&n.id))
    }
    fn group(doc: &Document, g: &Group, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth + 1);
        out.push_str(&format!(
            "{pad}subgraph {}[{}]\n",
            mermaid_id(&g.id),
            mermaid_text(g.display())
        ));
        let (nodes, groups) = doc.members(Some(&g.id));
        for n in nodes {
            out.push_str(&format!("{pad}  {}\n", node(n)));
        }
        for c in groups {
            group(doc, c, depth + 1, out);
        }
        out.push_str(&format!("{pad}end\n"));
    }
    let (nodes, groups) = doc.members(None);
    for n in nodes {
        out.push_str(&format!("  {}\n", node(n)));
    }
    for g in groups {
        group(doc, g, 0, &mut out);
    }
    for e in &doc.edges {
        let core = match (e.line, e.arrow) {
            (Line::Dashed | Line::Dotted, Arrow::None) => "-.-",
            (Line::Dashed | Line::Dotted, Arrow::Both) => "<-.->",
            (Line::Dashed | Line::Dotted, _) => "-.->",
            (Line::Thick, Arrow::None) => "===",
            (Line::Thick, Arrow::Both) => "<==>",
            (Line::Thick, _) => "==>",
            (_, Arrow::None) => "---",
            (_, Arrow::Both) => "<-->",
            _ => "-->",
        };
        let (from, to) = if e.arrow == Arrow::Start {
            (&e.to, &e.from)
        } else {
            (&e.from, &e.to)
        };
        let label = e
            .label
            .as_ref()
            .map(|l| format!("|{}|", mermaid_text(l)))
            .unwrap_or_default();
        out.push_str(&format!(
            "  {} {core}{label} {}\n",
            mermaid_id(from),
            mermaid_id(to)
        ));
    }
    for (k, c) in doc
        .comments
        .iter()
        .filter(|c| c.status == Status::Open)
        .enumerate()
    {
        if k == 0 {
            out.push_str("  %% review comments\n");
        }
        let on =
            c.on.as_deref()
                .map(|o| format!(" on {o}"))
                .unwrap_or_default();
        out.push_str(&format!(
            "  %% {}{on}: {}\n",
            c.id,
            c.text.replace('\n', " ")
        ));
    }
    out
}

pub fn looks_like(src: &str) -> bool {
    src.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%") && *l != "---" && !l.starts_with("title:"))
        .is_some_and(|l| {
            let w = l.split_whitespace().next().unwrap_or("");
            w == "flowchart" || w == "graph"
        })
}

struct Scan<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Scan<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }
    fn ws(&mut self) {
        let r = self.rest();
        self.i += r.len() - r.trim_start().len();
    }
    fn ident(&mut self) -> Option<String> {
        let r = self.rest();
        let chars: Vec<(usize, char)> = r.char_indices().collect();
        let w = |c: char| c.is_alphanumeric() || c == '_';
        let mut end = 0;
        for (k, &(i, c)) in chars.iter().enumerate() {
            if w(c) || (c == '-' && end > 0 && chars.get(k + 1).is_some_and(|&(_, n)| w(n))) {
                end = i + c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            return None;
        }
        self.i += end;
        Some(r[..end].to_string())
    }
}

fn clean(t: &str) -> String {
    let t = t.trim();
    let t = if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        &t[1..t.len() - 1]
    } else {
        t
    };
    t.replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("#quot;", "\"")
        .trim()
        .to_string()
}

const SHAPES: &[(&str, &str, Shape)] = &[
    ("([", "])", Shape::Pill),
    ("((", "))", Shape::Circle),
    ("[(", ")]", Shape::Cylinder),
    ("[/", "/]", Shape::Parallelogram),
    ("[\\", "\\]", Shape::Parallelogram),
    ("{{", "}}", Shape::Hexagon),
    ("[", "]", Shape::Box),
    ("(", ")", Shape::Rounded),
    ("{", "}", Shape::Diamond),
    (">", "]", Shape::Box),
];

pub fn import(src: &str) -> Result<Document, Vec<Diagnostic>> {
    let mut doc = Document::default();
    let mut errors = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut in_front = false;
    let mut seen_header = false;
    for (ln, raw) in src.lines().enumerate() {
        let line = raw.trim().trim_end_matches(';');
        let ln = ln + 1;
        if line == "---" {
            in_front = !in_front && !seen_header;
            continue;
        }
        if in_front {
            if let Some(t) = line.strip_prefix("title:") {
                doc.title = Some(t.trim().to_string());
            }
            continue;
        }
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        match first {
            "flowchart" | "graph" => {
                seen_header = true;
                if let Some(d) = line.split_whitespace().nth(1) {
                    doc.settings.direction = Direction::parse(d).unwrap_or_default();
                }
                continue;
            }
            "classDef" | "class" | "style" | "linkStyle" | "click" | "direction" => continue,
            "end" => {
                stack.pop();
                continue;
            }
            "subgraph" => {
                let rest = line["subgraph".len()..].trim();
                let mut sc = Scan { s: rest, i: 0 };
                let (id, label) = match sc.ident() {
                    Some(id) if sc.rest().trim_start().starts_with('[') => {
                        let r = sc.rest().trim();
                        (id, clean(r.trim_start_matches('[').trim_end_matches(']')))
                    }
                    Some(id) if sc.rest().trim().is_empty() => (id.clone(), String::new()),
                    _ => (slug(&clean(rest)), clean(rest)),
                };
                let id = if doc.group(&id).is_some() {
                    id
                } else {
                    doc.fresh_id(&id)
                };
                if doc.group(&id).is_none() {
                    doc.groups.push(Group {
                        id: id.clone(),
                        label: if label == id { String::new() } else { label },
                        parent: stack.last().cloned(),
                        color: None,
                    });
                }
                stack.push(id);
                continue;
            }
            _ => {}
        }
        if let Err(e) = statement(&mut doc, line, stack.last()) {
            errors.push(Diagnostic::new(ln, e));
        }
    }
    doc.normalize();
    if errors.is_empty() {
        Ok(doc)
    } else {
        Err(errors)
    }
}

fn node_ref(doc: &mut Document, sc: &mut Scan, group: Option<&String>) -> Result<String, String> {
    sc.ws();
    let id = sc
        .ident()
        .ok_or_else(|| format!("expected a node at `{}`", sc.rest()))?;
    let id = if crate::dsl::is_ident(&id) {
        id
    } else {
        slug(&id)
    };
    let mut label = None;
    let mut shape = None;
    for (open, close, sh) in SHAPES {
        if sc.rest().starts_with(open) {
            let body = &sc.rest()[open.len()..];
            let end = if body.starts_with('"') {
                body[1..]
                    .find('"')
                    .map(|q| q + 2)
                    .and_then(|q| body[q..].find(close).map(|c| c + q))
            } else {
                body.find(close)
            };
            let end = end.ok_or_else(|| format!("unclosed `{open}`"))?;
            label = Some(clean(&body[..end]));
            shape = Some(*sh);
            sc.i += open.len() + end + close.len();
            break;
        }
    }
    if sc.rest().starts_with(":::") {
        sc.i += 3;
        sc.ident();
    }
    if doc.node(&id).is_none() && doc.group(&id).is_none() {
        let mut n = Node::new(&id);
        n.group = group.cloned();
        doc.nodes.push(n);
    }
    if let Some(n) = doc.node_mut(&id) {
        if let Some(l) = label {
            n.label = if l == id { String::new() } else { l };
        }
        if let Some(s) = shape {
            n.shape = s;
        }
    }
    Ok(id)
}

fn link(sc: &mut Scan) -> Option<(Arrow, Line, Option<String>)> {
    sc.ws();
    let r = sc.rest();
    let start = r.starts_with('<');
    let body_start = if start { 1 } else { 0 };
    let b = &r[body_start..];
    let lead: String = b
        .chars()
        .take_while(|c| matches!(c, '-' | '.' | '='))
        .collect();
    if lead.len() < 2 && !(lead == "-" && b[1..].starts_with('.')) {
        return None;
    }
    let mut used = body_start + lead.len();
    let mut label = None;
    let mut tokens = lead.clone();
    let after = &r[used..];
    if let Some(t) = after.strip_prefix(['>', 'o', 'x']) {
        let _ = t;
        tokens.push(after.chars().next().unwrap());
        used += 1;
    } else if after.starts_with(' ') && (lead == "--" || lead == "==" || lead == "-.") {
        // `-- text -->` form
        let closer = ["-->", "---", "==>", "===", ".->", ".-"]
            .iter()
            .filter_map(|c| after.find(c).map(|p| (p, *c)))
            .min_by_key(|(p, _)| *p);
        if let Some((p, c)) = closer {
            label = Some(clean(&after[..p]));
            tokens.push_str(c);
            used += p + c.len();
        }
    }
    sc.i += used;
    if sc.rest().starts_with('|') {
        if let Some(end) = sc.rest()[1..].find('|') {
            label = Some(clean(&sc.rest()[1..end + 1]));
            sc.i += end + 2;
        }
    }
    let line = if tokens.contains('=') {
        Line::Thick
    } else if tokens.contains('.') {
        Line::Dashed
    } else {
        Line::Solid
    };
    let end = tokens.ends_with('>') || tokens.ends_with('o') || tokens.ends_with('x');
    let arrow = match (start, end) {
        (true, true) => Arrow::Both,
        (false, true) => Arrow::End,
        (true, false) => Arrow::Start,
        _ => Arrow::None,
    };
    Some((arrow, line, label.filter(|l| !l.is_empty())))
}

fn statement(doc: &mut Document, line: &str, group: Option<&String>) -> Result<(), String> {
    let mut sc = Scan { s: line, i: 0 };
    let mut prev = vec![node_ref(doc, &mut sc, group)?];
    loop {
        sc.ws();
        while sc.rest().starts_with('&') {
            sc.i += 1;
            prev.push(node_ref(doc, &mut sc, group)?);
            sc.ws();
        }
        if sc.rest().is_empty() {
            // a bare node list inside a subgraph moves those nodes into it
            if let Some(g) = group {
                for id in &prev {
                    if let Some(n) = doc.node_mut(id) {
                        n.group = Some(g.clone());
                    }
                }
            }
            return Ok(());
        }
        let (arrow, line, label) =
            link(&mut sc).ok_or_else(|| format!("unexpected `{}`", sc.rest()))?;
        let mut next = vec![node_ref(doc, &mut sc, group)?];
        sc.ws();
        while sc.rest().starts_with('&') {
            sc.i += 1;
            next.push(node_ref(doc, &mut sc, group)?);
            sc.ws();
        }
        for a in &prev {
            for b in &next {
                let id = doc.fresh_edge_id(a, b);
                doc.edges.push(Edge {
                    id,
                    from: a.clone(),
                    to: b.clone(),
                    label: label.clone(),
                    arrow,
                    line,
                    color: None,
                });
            }
        }
        prev = next;
        if sc.rest().trim().is_empty() {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_a_flowchart() {
        let src = r#"
flowchart TD
  A[Start] --> B{Is it?}
  B -->|Yes| C[(Database)]
  B -- No --> D([Done])
  C -.-> D
  E & F ==> G
  subgraph backend [Backend]
    C
    H((hub))
  end
  %% comment
  classDef red fill:#f00
"#;
        assert!(looks_like(src));
        let d = import(src).unwrap();
        assert_eq!(d.settings.direction, Direction::Down);
        assert_eq!(d.node("B").unwrap().shape, Shape::Diamond);
        assert_eq!(d.node("C").unwrap().shape, Shape::Cylinder);
        assert_eq!(d.node("C").unwrap().group.as_deref(), Some("backend"));
        assert_eq!(d.edge("B->C").unwrap().label.as_deref(), Some("Yes"));
        assert_eq!(d.edge("B->D").unwrap().label.as_deref(), Some("No"));
        assert_eq!(d.edge("C->D").unwrap().line, Line::Dashed);
        assert_eq!(d.edge("E->G").unwrap().line, Line::Thick);
        assert!(d.edge("F->G").is_some());
    }

    #[test]
    fn round_trips_through_mermaid() {
        let d = crate::dsl::parse(
            "direction: down\ngroup g: G {\n a: Alpha [cylinder]\n}\na -> b: x\nb .. c\nc <=> a",
        )
        .unwrap();
        let m = export(&d);
        let back = import(&m).unwrap();
        assert_eq!(back.edges.len(), 3);
        assert_eq!(back.node("a").unwrap().shape, Shape::Cylinder);
        assert_eq!(back.node("a").unwrap().group.as_deref(), Some("g"));
        assert_eq!(back.edge("c->a").unwrap().arrow, Arrow::Both);
        assert_eq!(back.edge("b->c").unwrap().arrow, Arrow::None);
    }
}
