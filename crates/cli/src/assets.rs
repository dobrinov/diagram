//! Web assets compiled into the binary. Set DIAGRAM_WEB_DIR to serve them
//! from disk instead while hacking on the frontend.
use std::borrow::Cow;

macro_rules! web {
    ($p:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/", $p))
    };
}

pub const INDEX_HTML: &[u8] = web!("index.html");
pub const APP_JS: &[u8] = web!("app.js");
pub const EDITOR_JS: &[u8] = web!("editor.js");
pub const EDITOR_CSS: &[u8] = web!("editor.css");
pub const EMBED_JS: &[u8] = web!("embed.js");
pub const FAVICON: &[u8] = web!("favicon.svg");
pub const GLUE_JS: &[u8] = web!("pkg/diagram_wasm.js");
pub const WASM: &[u8] = web!("pkg/diagram_wasm_bg.wasm");

pub fn get(path: &str) -> Option<(Cow<'static, [u8]>, &'static str)> {
    let (bytes, ct): (&'static [u8], &str) = match path {
        "/" | "/index.html" => (INDEX_HTML, "text/html; charset=utf-8"),
        "/app.js" => (APP_JS, "text/javascript; charset=utf-8"),
        "/editor.js" => (EDITOR_JS, "text/javascript; charset=utf-8"),
        "/editor.css" => (EDITOR_CSS, "text/css; charset=utf-8"),
        "/embed.js" => (EMBED_JS, "text/javascript; charset=utf-8"),
        "/favicon.svg" => (FAVICON, "image/svg+xml"),
        "/pkg/diagram_wasm.js" => (GLUE_JS, "text/javascript; charset=utf-8"),
        "/pkg/diagram_wasm_bg.wasm" => (WASM, "application/wasm"),
        _ => return None,
    };
    if let Some(dir) = std::env::var_os("DIAGRAM_WEB_DIR") {
        let rel = if path == "/" {
            "index.html"
        } else {
            path.trim_start_matches('/')
        };
        if let Ok(b) = std::fs::read(std::path::Path::new(&dir).join(rel)) {
            return Some((Cow::Owned(b), ct));
        }
    }
    Some((Cow::Borrowed(bytes), ct))
}

pub fn text(b: &[u8]) -> &str {
    std::str::from_utf8(b).unwrap_or("")
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn b64() {
        assert_eq!(super::base64(b"hello!?"), "aGVsbG8hPw==");
    }
}
