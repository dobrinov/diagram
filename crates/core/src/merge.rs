//! Three-way merge of boards: what two people did to the same board since a
//! common version, element by element and field by field.
//!
//! An element changed on one side only takes that change; added on one side,
//! it is added; removed on one side and left alone on the other, it goes.
//! When both sides changed the same element, their changes to different
//! fields are combined; only the same field changed two ways (or an element
//! changed on one side and removed on the other) is a [`Conflict`]. The
//! merged board keeps "yours" for each conflict until [`resolve`] picks.
use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::model::*;

/// Something both sides changed in different ways.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Conflict {
    /// `node`, `edge`, `group`, `comment`, `asset`, `theme`, `canvas` or
    /// `settings`.
    pub kind: &'static str,
    pub id: String,
    /// The canvas it is in (ids from the top board; empty: the top board).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub canvas: Vec<String>,
    /// Its name, to show (a label, a title, the id).
    pub label: String,
    /// The fields changed both ways (empty: changed on one side, removed on
    /// the other).
    pub fields: Vec<String>,
    /// The element as it was, as yours and theirs have it (None: removed),
    /// with the other side's changes to other fields already in.
    pub base: Option<Value>,
    pub ours: Option<Value>,
    pub theirs: Option<Value>,
}

impl Conflict {
    /// How a pick names it: `node:api`, `canvas/payments/node:api`.
    pub fn key(&self) -> String {
        let mut k = String::new();
        for c in &self.canvas {
            k.push_str(&format!("canvas/{c}/"));
        }
        k.push_str(&format!("{}:{}", self.kind, self.id));
        k
    }
}

#[derive(Debug, Clone)]
pub struct Merged {
    /// The merged board, with yours for every conflict.
    pub doc: Document,
    pub conflicts: Vec<Conflict>,
}

/// Merge `ours` and `theirs`, both changed from `base`.
pub fn merge3(base: &Document, ours: &Document, theirs: &Document) -> Merged {
    let mut conflicts = Vec::new();
    let doc = merge_doc(base, ours, theirs, &[], &mut conflicts);
    Merged { doc, conflicts }
}

fn merge_doc(
    b: &Document,
    o: &Document,
    t: &Document,
    path: &[String],
    out: &mut Vec<Conflict>,
) -> Document {
    // the title, description and settings merge as fields of one element
    let head = |d: &Document| {
        let mut m = Map::new();
        m.insert("title".into(), json(&d.title));
        m.insert("description".into(), json(&d.description));
        m.insert("settings".into(), json(&d.settings));
        Value::Object(m)
    };
    let (hb, ho, ht) = (head(b), head(o), head(t));
    let h = match merge_value(Some(&hb), Some(&ho), Some(&ht)) {
        Ok(v) => v,
        Err((ours, theirs, fields)) => {
            out.push(Conflict {
                kind: "settings",
                id: "settings".into(),
                canvas: path.to_vec(),
                label: "Title and settings".into(),
                fields,
                base: Some(hb),
                ours: Some(ours.clone()),
                theirs: Some(theirs),
            });
            ours
        }
    };
    let mut doc = Document {
        title: from(&h["title"]).unwrap_or_default(),
        description: from(&h["description"]).unwrap_or_default(),
        settings: from(&h["settings"]).unwrap_or_default(),
        ..Default::default()
    };
    let label_node = |n: &Node| n.display().to_string();
    doc.nodes = merge_list(
        &b.nodes,
        &o.nodes,
        &t.nodes,
        |n| &n.id,
        label_node,
        "node",
        path,
        out,
    );
    doc.edges = merge_list(
        &b.edges,
        &o.edges,
        &t.edges,
        |e| &e.id,
        |e| {
            e.label
                .clone()
                .unwrap_or_else(|| format!("{} → {}", e.from, e.to))
        },
        "edge",
        path,
        out,
    );
    doc.groups = merge_list(
        &b.groups,
        &o.groups,
        &t.groups,
        |g| &g.id,
        |g| g.display().to_string(),
        "group",
        path,
        out,
    );
    doc.comments = merge_list(
        &b.comments,
        &o.comments,
        &t.comments,
        |c| &c.id,
        |c| c.text.chars().take(40).collect(),
        "comment",
        path,
        out,
    );
    doc.themes = merge_list(
        &b.themes,
        &o.themes,
        &t.themes,
        |x| &x.id,
        |x| x.id.clone(),
        "theme",
        path,
        out,
    );
    doc.assets = merge_list(
        &b.assets,
        &o.assets,
        &t.assets,
        |a| &a.id,
        |a| a.id.clone(),
        "asset",
        path,
        out,
    );
    // canvases changed on both sides merge in turn
    let ids = union_ids(
        &b.canvases.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        &o.canvases.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        &t.canvases.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
    );
    for id in ids {
        let (cb, co, ct) = (b.canvas(&id), o.canvas(&id), t.canvas(&id));
        let mut sub = path.to_vec();
        sub.push(id.clone());
        let pick = match (cb, co, ct) {
            (_, Some(x), Some(y)) if x == y => Some(x.clone()),
            (Some(z), Some(x), Some(y)) => Some(Canvas {
                id: id.clone(),
                doc: merge_doc(&z.doc, &x.doc, &y.doc, &sub, out),
            }),
            (None, Some(x), Some(y)) => Some(Canvas {
                id: id.clone(),
                doc: merge_doc(&Document::default(), &x.doc, &y.doc, &sub, out),
            }),
            (Some(z), Some(x), None) | (Some(z), None, Some(x)) if z == x => None,
            (Some(z), Some(x), None) | (Some(z), None, Some(x)) => {
                let ours_side = co.is_some();
                out.push(Conflict {
                    kind: "canvas",
                    id: id.clone(),
                    canvas: path.to_vec(),
                    label: x.display().to_string(),
                    fields: Vec::new(),
                    base: Some(json(z)),
                    ours: co.map(json),
                    theirs: ct.map(json),
                });
                ours_side.then(|| x.clone())
            }
            (_, Some(x), None) | (_, None, Some(x)) => Some(x.clone()),
            _ => None,
        };
        doc.canvases.extend(pick);
    }
    doc
}

fn json<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn from<T: DeserializeOwned>(v: &Value) -> Option<T> {
    serde_json::from_value(v.clone()).ok()
}

/// Every id, in the order yours has them, with theirs' new ones after the
/// one they follow there.
fn union_ids(b: &[&str], o: &[&str], t: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = o.iter().map(|s| s.to_string()).collect();
    for (k, id) in t.iter().enumerate() {
        if out.iter().any(|x| x == id) {
            continue;
        }
        // after the nearest one before it in theirs that is placed
        let at = t[..k]
            .iter()
            .rev()
            .find_map(|p| out.iter().position(|x| x == p))
            .map_or(0, |p| p + 1);
        out.insert(at, id.to_string());
    }
    // removed on both sides: gone
    for id in b {
        if !o.contains(id) && !t.contains(id) {
            out.retain(|x| x != id);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn merge_list<T>(
    b: &[T],
    o: &[T],
    t: &[T],
    id: fn(&T) -> &String,
    label: impl Fn(&T) -> String,
    kind: &'static str,
    path: &[String],
    out: &mut Vec<Conflict>,
) -> Vec<T>
where
    T: Serialize + DeserializeOwned + Clone + PartialEq,
{
    let find = |xs: &[T], k: &str| xs.iter().find(|x| id(x) == k).cloned();
    let ids = union_ids(
        &b.iter().map(|x| id(x).as_str()).collect::<Vec<_>>(),
        &o.iter().map(|x| id(x).as_str()).collect::<Vec<_>>(),
        &t.iter().map(|x| id(x).as_str()).collect::<Vec<_>>(),
    );
    let mut res = Vec::new();
    for k in ids {
        let (xb, xo, xt) = (find(b, &k), find(o, &k), find(t, &k));
        let (vb, vo, vt) = (
            xb.as_ref().map(json),
            xo.as_ref().map(json),
            xt.as_ref().map(json),
        );
        let name = xo
            .as_ref()
            .or(xt.as_ref())
            .or(xb.as_ref())
            .map(&label)
            .unwrap_or_default();
        let conflict =
            |out: &mut Vec<Conflict>, ours: Option<Value>, theirs: Option<Value>, fields| {
                out.push(Conflict {
                    kind,
                    id: k.clone(),
                    canvas: path.to_vec(),
                    label: name.clone(),
                    fields,
                    base: vb.clone(),
                    ours,
                    theirs,
                })
            };
        match (&vb, &vo, &vt) {
            (_, Some(x), Some(y)) if x == y => res.extend(xo),
            (_, Some(_), Some(_)) => match merge_value(vb.as_ref(), vo.as_ref(), vt.as_ref()) {
                Ok(v) => res.extend(from::<T>(&v).or(xo)),
                Err((ours, theirs, fields)) => {
                    res.extend(from::<T>(&ours).or(xo));
                    conflict(out, Some(ours), Some(theirs), fields);
                }
            },
            // removed on one side: gone, unless the other side changed it
            (Some(z), Some(x), None) => {
                if z != x {
                    conflict(out, vo.clone(), None, Vec::new());
                    res.extend(xo);
                }
            }
            (Some(z), None, Some(y)) => {
                if z != y {
                    conflict(out, None, vt.clone(), Vec::new());
                }
            }
            (None, Some(_), None) => res.extend(xo),
            (None, None, Some(_)) => res.extend(xt),
            _ => {}
        }
    }
    res
}

/// Merge one element's fields: `Ok(merged)`, or `Err((yours, theirs,
/// fields))` with each side's value for the fields changed both ways (and
/// the other changes in both).
fn merge_value(
    b: Option<&Value>,
    o: Option<&Value>,
    t: Option<&Value>,
) -> Result<Value, (Value, Value, Vec<String>)> {
    let obj = |v: Option<&Value>| v.and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let (mb, mo, mt) = (obj(b), obj(o), obj(t));
    let mut keys: BTreeMap<String, ()> = BTreeMap::new();
    for m in [&mb, &mo, &mt] {
        for k in m.keys() {
            keys.insert(k.clone(), ());
        }
    }
    let (mut ours, mut theirs) = (Map::new(), Map::new());
    let mut fields = Vec::new();
    for k in keys.keys() {
        let (vb, vo, vt) = (mb.get(k), mo.get(k), mt.get(k));
        let (a, z) = if vo == vt || vt == vb {
            (vo, vo)
        } else if vo == vb {
            (vt, vt)
        } else {
            fields.push(k.clone());
            (vo, vt)
        };
        if let Some(v) = a {
            ours.insert(k.clone(), v.clone());
        }
        if let Some(v) = z {
            theirs.insert(k.clone(), v.clone());
        }
    }
    if fields.is_empty() {
        Ok(Value::Object(ours))
    } else {
        Err((Value::Object(ours), Value::Object(theirs), fields))
    }
}

/// The merged board with a side picked for conflicts: `picks` maps a
/// conflict's [`Conflict::key`] to `"ours"` or `"theirs"` (yours when left
/// out).
pub fn resolve(m: &Merged, picks: &BTreeMap<String, String>) -> Document {
    let mut doc = m.doc.clone();
    for c in &m.conflicts {
        if picks.get(&c.key()).map(String::as_str) != Some("theirs") {
            continue;
        }
        let Some(d) = doc.at_mut(&c.canvas) else {
            continue;
        };
        put(d, c.kind, &c.id, c.theirs.as_ref());
    }
    doc
}

/// Put `v` (None: nothing) in place of element `id` of `kind`.
fn put(d: &mut Document, kind: &str, id: &str, v: Option<&Value>) {
    fn swap<T: DeserializeOwned + Clone>(
        xs: &mut Vec<T>,
        id: fn(&T) -> &String,
        k: &str,
        v: Option<&Value>,
    ) {
        let new: Option<T> = v.and_then(from);
        match (xs.iter().position(|x| id(x) == k), new) {
            (Some(i), Some(n)) => xs[i] = n,
            (Some(i), None) => {
                xs.remove(i);
            }
            (None, Some(n)) => xs.push(n),
            (None, None) => {}
        }
    }
    match kind {
        "node" => swap(&mut d.nodes, |n| &n.id, id, v),
        "edge" => swap(&mut d.edges, |e| &e.id, id, v),
        "group" => swap(&mut d.groups, |g| &g.id, id, v),
        "comment" => swap(&mut d.comments, |c| &c.id, id, v),
        "theme" => swap(&mut d.themes, |x| &x.id, id, v),
        "asset" => swap(&mut d.assets, |a| &a.id, id, v),
        "canvas" => swap(&mut d.canvases, |c| &c.id, id, v),
        "settings" => {
            if let Some(v) = v {
                d.title = from(&v["title"]).unwrap_or_default();
                d.description = from(&v["description"]).unwrap_or_default();
                d.settings = from(&v["settings"]).unwrap_or_default();
            }
        }
        _ => {}
    }
}

/// A conflict as one line for people and models: what each side has.
pub fn describe(c: &Conflict) -> String {
    let side = |v: &Option<Value>| -> String {
        match v {
            None => "removed it".into(),
            Some(v) if c.fields.is_empty() => format!("changed it ({})", compact(v)),
            Some(v) => c
                .fields
                .iter()
                .map(|f| format!("{f} = {}", compact(v.get(f).unwrap_or(&Value::Null))))
                .collect::<Vec<_>>()
                .join(", "),
        }
    };
    let place = if c.canvas.is_empty() {
        String::new()
    } else {
        format!(" (in canvas {})", c.canvas.join("/"))
    };
    format!(
        "{} `{}` “{}”{place}: yours {}; theirs {}",
        c.kind,
        c.id,
        c.label,
        side(&c.ours),
        side(&c.theirs)
    )
}

fn compact(v: &Value) -> String {
    match v {
        Value::Null => "none".into(),
        Value::String(s) => format!("“{s}”"),
        v => {
            let s = v.to_string();
            if s.chars().count() > 160 {
                format!("{}…", s.chars().take(160).collect::<String>())
            } else {
                s
            }
        }
    }
}

/// A prompt asking a model to settle the conflicts as `instruction` says:
/// it answers with a pick per conflict, and optional ops for anything else.
pub fn prompt(m: &Merged, instruction: &str) -> String {
    let mut s = String::from(
        "# Settle a merge conflict on a diagram\n\nTwo people changed the same board. Everything that did not clash is merged already. These elements were changed both ways:\n\n",
    );
    for c in &m.conflicts {
        s.push_str(&format!("- {} — key `{}`\n", describe(c), c.key()));
    }
    s.push_str("\n## The merged board (yours where they clash)\n\n```diagram\n");
    s.push_str(&crate::dsl::print_lean(&m.doc));
    s.push_str("```\n\n## What the person wants\n\n");
    s.push_str(instruction.trim());
    s.push_str(
        "\n\n## How to answer\n\nReply with one ```json block: `pick` maps each key above to \"ours\" or \"theirs\", and `ops` (optional) are edits to make after that, in the diagram ops format (add_node, update_node, remove, …), for wishes a pick alone cannot meet (keeping parts of both).\n\n```json\n{\"pick\": {\"node:api\": \"theirs\"}, \"ops\": []}\n```\n",
    );
    s
}

/// A model's answer to [`prompt`]: its picks and ops.
pub fn parse_answer(
    reply: &str,
) -> Result<(BTreeMap<String, String>, Vec<crate::ops::Op>), String> {
    let body = reply
        .find("```json")
        .map(|i| &reply[i + 7..])
        .map(|r| &r[..r.find("```").unwrap_or(r.len())])
        .unwrap_or(reply);
    let start = body.find('{').ok_or("no JSON in the reply")?;
    let end = body.rfind('}').ok_or("no JSON in the reply")?;
    let v: Value = serde_json::from_str(&body[start..=end]).map_err(|e| e.to_string())?;
    let mut picks = BTreeMap::new();
    if let Some(p) = v.get("pick").and_then(|p| p.as_object()) {
        for (k, side) in p {
            let side = side.as_str().unwrap_or("");
            if side != "ours" && side != "theirs" {
                return Err(format!("pick `{k}`: \"ours\" or \"theirs\", not {side:?}"));
            }
            picks.insert(k.clone(), side.to_string());
        }
    }
    let ops = match v.get("ops") {
        Some(o) if o.as_array().is_some_and(|a| !a.is_empty()) => {
            crate::ops::parse_ops(&serde_json::json!({ "ops": o }).to_string())?
        }
        _ => Vec::new(),
    };
    Ok((picks, ops))
}

/// A file git merged line by line, with conflict markers: (base, yours,
/// theirs), each side taking its part of every conflict. The base is there
/// when git wrote it (`merge.conflictStyle = diff3`). None without markers.
pub fn split_markers(text: &str) -> Option<(Option<String>, String, String)> {
    if !text.lines().any(|l| l.starts_with("<<<<<<<")) {
        return None;
    }
    let (mut b, mut o, mut t) = (String::new(), String::new(), String::new());
    let mut has_base = false;
    // 0: both, 1: ours, 2: base, 3: theirs
    let mut part = 0;
    for l in text.lines() {
        if l.starts_with("<<<<<<<") {
            part = 1;
        } else if l.starts_with("|||||||") && part == 1 {
            part = 2;
            has_base = true;
        } else if l.starts_with("=======") && (part == 1 || part == 2) {
            part = 3;
        } else if l.starts_with(">>>>>>>") && part == 3 {
            part = 0;
        } else {
            let line = format!("{l}\n");
            match part {
                0 => {
                    b.push_str(&line);
                    o.push_str(&line);
                    t.push_str(&line);
                }
                1 => o.push_str(&line),
                2 => b.push_str(&line),
                _ => t.push_str(&line),
            }
        }
    }
    Some((has_base.then_some(b), o, t))
}

/// The merge comment `diagram merge` puts on top of a file it could not
/// merge cleanly, taken off.
pub fn strip_note(text: &str) -> &str {
    match text.strip_prefix("# merge: ") {
        Some(rest) => rest.split_once('\n').map_or("", |(_, r)| r),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Document {
        crate::dsl::parse(s).unwrap()
    }

    #[test]
    fn changes_on_both_sides_merge_and_clashes_are_conflicts() {
        let base = d("title: Shop\napi: Orders API\ndb: Postgres\napi -> db\nold: Old thing\n");
        // yours: colour the API, add a cache, remove `old`
        let ours = d("title: Shop\napi: Orders API [blue]\ndb: Postgres\ncache: Redis\napi -> db\napi -> cache\n");
        // theirs: rename the API's detail, relabel the db, add a queue
        let theirs = d("title: Shop\napi: Orders API [detail: Rust]\ndb: Postgres 16\nqueue: Jobs\napi -> db\nold: Old thing\n");
        let m = merge3(&base, &ours, &theirs);
        assert!(m.conflicts.is_empty(), "{:?}", m.conflicts);
        let api = m.doc.node("api").unwrap();
        assert_eq!(
            (api.color.as_deref(), api.detail.as_deref()),
            (Some("blue"), Some("Rust"))
        );
        assert_eq!(m.doc.node("db").unwrap().label, "Postgres 16");
        assert!(m.doc.node("cache").is_some() && m.doc.node("queue").is_some());
        assert!(m.doc.node("old").is_none());
        assert!(m.doc.edge("api->cache").is_some());
        let ids: Vec<&str> = m.doc.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["api", "db", "queue", "cache"]);

        // the same field two ways, and changed-vs-removed
        let ours =
            d("title: Shop\napi: Orders API [red]\ndb: Postgres\napi -> db\nold: Older thing\n");
        let theirs = d("title: Shop\napi: Orders API [green]\ndb: Postgres\napi -> db\n");
        let m = merge3(&base, &ours, &theirs);
        let keys: Vec<String> = m.conflicts.iter().map(|c| c.key()).collect();
        assert_eq!(keys, ["node:api", "node:old"]);
        assert_eq!(m.conflicts[0].fields, ["color"]);
        assert!(describe(&m.conflicts[0]).contains("yours color = “red”; theirs color = “green”"));
        assert_eq!(m.doc.node("api").unwrap().color.as_deref(), Some("red"));
        let picks: BTreeMap<String, String> = [
            ("node:api".into(), "theirs".into()),
            ("node:old".into(), "theirs".into()),
        ]
        .into();
        let r = resolve(&m, &picks);
        assert_eq!(r.node("api").unwrap().color.as_deref(), Some("green"));
        assert!(r.node("old").is_none());
        // a model's answer
        let (p, ops) = parse_answer("Sure:\n```json\n{\"pick\": {\"node:api\": \"theirs\"}, \"ops\": [{\"op\":\"update_node\",\"id\":\"api\",\"detail\":\"both\"}]}\n```").unwrap();
        assert_eq!(
            (p.get("node:api").map(String::as_str), ops.len()),
            (Some("theirs"), 1)
        );
        assert!(prompt(&m, "Keep their colour").contains("key `node:api`"));
    }

    #[test]
    fn git_markers_split_into_both_sides() {
        let text = "title: T\n<<<<<<< HEAD\na: A [red]\n||||||| base\na: A\n=======\na: A [green]\n>>>>>>> theirs\nb: B\n";
        let (b, o, t) = split_markers(text).unwrap();
        assert_eq!(b.as_deref(), Some("title: T\na: A\nb: B\n"));
        assert_eq!(o, "title: T\na: A [red]\nb: B\n");
        assert_eq!(t, "title: T\na: A [green]\nb: B\n");
        assert!(split_markers("a -> b\n").is_none());
        assert_eq!(
            strip_note("# merge: 1 conflict …\ntitle: T\n"),
            "title: T\n"
        );
    }

    #[test]
    fn canvases_merge_inside() {
        let base = d("pay: Pay [canvas: p]\n\ncanvas p: P {\n  a: A\n}\n");
        let ours = d("pay: Pay [canvas: p]\n\ncanvas p: P {\n  a: A [blue]\n}\n");
        let theirs = d("pay: Pay [canvas: p]\n\ncanvas p: P {\n  a: A\n  b: B\n}\n");
        let m = merge3(&base, &ours, &theirs);
        assert!(m.conflicts.is_empty());
        let p = &m.doc.canvas("p").unwrap().doc;
        assert_eq!(p.node("a").unwrap().color.as_deref(), Some("blue"));
        assert!(p.node("b").is_some());
        let theirs = d("pay: Pay [canvas: p]\n\ncanvas p: P {\n  a: A [red]\n}\n");
        let m = merge3(&base, &ours, &theirs);
        assert_eq!(m.conflicts[0].key(), "canvas/p/node:a");
        let r = resolve(
            &m,
            &[("canvas/p/node:a".to_string(), "theirs".to_string())].into(),
        );
        assert_eq!(
            r.canvas("p")
                .unwrap()
                .doc
                .node("a")
                .unwrap()
                .color
                .as_deref(),
            Some("red")
        );
    }
}
