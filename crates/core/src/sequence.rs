//! Sequence diagrams: a group whose nodes are participants, side by side, and
//! whose messages are the arrows between them, top to bottom in the order
//! they are written.
//!
//! ```text
//! sequence login: Sign in {
//!   user: User [person]
//!   web: Web app
//!   api: API
//!   user -> web: opens /login
//!   web -> api: POST /session
//!   api ..> web: 201 + token
//! }
//! ```
//!
//! The layout puts the participants in a row and leaves room for the
//! messages; [`message_points`] draws each message straight across from one
//! lifeline to the other at its step.
use crate::layout::Layout;
use crate::model::{Document, Edge, Group, Node};

/// Space under the participants before the first message, between
/// messages, and after the last.
pub const TOP: f64 = 36.0;
pub const STEP: f64 = 46.0;
pub const BOTTOM: f64 = 24.0;
/// The least space between participants.
pub const GAP: f64 = 48.0;

/// The participants of sequence `g`, left to right (document order).
pub fn participants_of<'a>(doc: &'a Document, g: &str) -> Vec<&'a Node> {
    doc.nodes
        .iter()
        .filter(|n| n.group.as_deref() == Some(g))
        .collect()
}

/// The sequence an arrow is a message of: both ends are participants of it.
pub fn sequence_of<'a>(doc: &'a Document, e: &Edge) -> Option<&'a Group> {
    let g = doc.node(&e.from)?.group.as_deref()?;
    if doc.node(&e.to)?.group.as_deref() != Some(g) {
        return None;
    }
    doc.group(g).filter(|g| g.sequence)
}

/// The messages of sequence `g`, top to bottom.
pub fn messages_of<'a>(doc: &'a Document, g: &str) -> Vec<&'a Edge> {
    doc.edges
        .iter()
        .filter(|e| sequence_of(doc, e).is_some_and(|s| s.id == g))
        .collect()
}

/// The line a message is drawn along, once the participants are laid out:
/// from one lifeline straight to the other at the message's step (a message
/// to itself loops out to the right and back).
pub fn message_points(doc: &Document, lay: &Layout, e: &Edge) -> Option<Vec<(f64, f64)>> {
    let g = sequence_of(doc, e)?;
    let k = messages_of(doc, &g.id).iter().position(|m| m.id == e.id)?;
    let head = participants_of(doc, &g.id)
        .iter()
        .filter_map(|n| lay.rect(&n.id))
        .map(|r| r.bottom())
        .fold(f64::NEG_INFINITY, f64::max);
    let (a, b) = (lay.rect(&e.from)?, lay.rect(&e.to)?);
    let y = head + TOP + k as f64 * STEP;
    Some(if e.from == e.to {
        let x = a.cx();
        vec![
            (x, y - 9.0),
            (x + 36.0, y - 9.0),
            (x + 36.0, y + 9.0),
            (x + 2.0, y + 9.0),
        ]
    } else {
        vec![(a.cx(), y), (b.cx(), y)]
    })
}

#[cfg(test)]
mod tests {
    const SRC: &str = "sequence login: Sign in {\n  user: User\n  web: Web app\n  api: API\n  user -> web: opens /login\n  web -> api: POST /session\n  api -> api: sign\n  api ..> web: 201\n}\n\nweb -> cdn\n";

    #[test]
    fn messages_run_top_to_bottom_between_lifelines() {
        let mut d = crate::dsl::parse(SRC).unwrap();
        assert!(d.group("login").unwrap().sequence);
        let ids: Vec<&str> = super::messages_of(&d, "login")
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, ["user->web", "web->api", "api->api", "api->web"]);
        // `web -> cdn` leaves the diagram: an ordinary arrow, written outside it
        assert!(super::sequence_of(&d, d.edge("web->cdn").unwrap()).is_none());
        let text = crate::dsl::print(&d);
        assert!(
            text.starts_with("sequence login: Sign in {\n  user: User\n"),
            "{text}"
        );
        assert!(
            text.contains("  api ..> web: 201\n}\n\nweb -> cdn\n"),
            "{text}"
        );
        assert_eq!(crate::dsl::parse(&text).unwrap(), d);
        // participants in a row; each message lower than the one before
        let lay = crate::layout::layout(&d);
        let r = |id: &str| lay.rect(id).unwrap();
        assert!(r("user").right() < r("web").x && r("web").right() < r("api").x);
        let pts = |id: &str| super::message_points(&d, &lay, d.edge(id).unwrap()).unwrap();
        assert_eq!(
            pts("user->web"),
            [
                (r("user").cx(), pts("user->web")[0].1),
                (r("web").cx(), pts("user->web")[0].1)
            ]
        );
        assert!(pts("web->api")[0].1 > pts("user->web")[0].1);
        assert!(pts("api->web")[0].1 > pts("api->api")[3].1);
        assert!(lay.rect("login").unwrap().bottom() > pts("api->web")[0].1);
        let svg = crate::render::render(&d, &Default::default()).svg;
        assert!(svg.contains("dg-lifelines"));
        // an edit keeps the file as written; a new message goes into the block
        let old = d.clone();
        let mut new = d.clone();
        crate::ops::apply(
            &mut new,
            &crate::ops::parse_ops(
                r#"{"op":"add_edge","from":"web","to":"api","label":"GET /me"}"#,
            )
            .unwrap(),
        )
        .unwrap();
        let patched = crate::patch::patch(SRC, &old, &new);
        assert!(
            patched.contains("  api ..> web: 201\n  web -> api: GET /me\n}\n\nweb -> cdn\n"),
            "{patched}"
        );
        // reorder: the reply goes first
        crate::ops::apply(
            &mut d,
            &crate::ops::parse_ops(r#"{"op":"update_edge","id":"api->web","index":0}"#).unwrap(),
        )
        .unwrap();
        let ids: Vec<&str> = super::messages_of(&d, "login")
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, ["api->web", "user->web", "web->api", "api->api"]);
        crate::ops::apply(
            &mut d,
            &crate::ops::parse_ops(r#"{"op":"update_edge","id":"api->web","index":9}"#).unwrap(),
        )
        .unwrap();
        let ids: Vec<&str> = super::messages_of(&d, "login")
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, ["user->web", "web->api", "api->api", "api->web"]);
        // prompts: numbered steps
        let md = crate::prompt::selection(&d, &["login".to_string()]);
        assert!(
            md.contains("Participants: User, Web app, API.\n\n1. User → Web app: opens /login\n"),
            "{md}"
        );
        assert!(md.contains("4. API → Web app: 201 (reply)"), "{md}");
    }
}
