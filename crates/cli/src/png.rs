//! `diagram png`: rasterise the SVG with resvg. The SVG uses literal colours,
//! so nothing needs translating; only the font stack is resolved here.
use std::sync::Arc;

use resvg::{tiny_skia, usvg};

/// Largest side of the image in pixels; bigger diagrams are scaled down to fit.
const MAX_SIDE: f32 = 16384.0;

/// Sans-serif fonts to draw with, best first (the SVG asks for a CSS font
/// stack that only browsers resolve).
const SANS: &[&str] = &[
    "Inter",
    "Helvetica Neue",
    "Helvetica",
    "Arial",
    "Segoe UI",
    "DejaVu Sans",
    "Liberation Sans",
    "Noto Sans",
];

pub fn render(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let mut db = usvg::fontdb::Database::new();
    db.load_system_fonts();
    let families: Vec<String> = db
        .faces()
        .flat_map(|f| f.families.iter().map(|(n, _)| n.clone()))
        .collect();
    if let Some(sans) = SANS.iter().find(|m| families.iter().any(|f| f == *m)) {
        db.set_sans_serif_family(*sans);
    }
    let svg = svg.replace(
        &format!(
            "font-family=\"{}\"",
            diagram_core::text::esc(diagram_core::text::FONT)
        ),
        "font-family=\"sans-serif\"",
    );
    let opt = usvg::Options {
        fontdb: Arc::new(db),
        ..Default::default()
    };
    let tree =
        usvg::Tree::from_str(&svg, &opt).map_err(|e| format!("could not read the SVG: {e}"))?;
    let size = tree.size();
    let scale = scale.min(MAX_SIDE / size.width().max(size.height()));
    let (w, h) = (
        (size.width() * scale).ceil() as u32,
        (size.height() * scale).ceil() as u32,
    );
    let mut pixmap =
        tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("the diagram is too large for a PNG")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| format!("could not encode the PNG: {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_a_png() {
        let doc = diagram_core::parse("a -> b: hello\nc [sticky]").unwrap();
        let svg = diagram_core::render_svg(&doc, &Default::default());
        let png = super::render(&svg, 1.0).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }
}
