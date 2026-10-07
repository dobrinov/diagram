/* diagram browser library (part of diagram.embed.js, which also inlines the
 * WASM module and the editor).
 *
 *   await Diagram.ready()
 *   Diagram.render(src, {theme, comments, title, transparent}) -> Promise<svg string>
 *   Diagram.convert(src, "dsl"|"json"|"mermaid"|"brief") -> Promise<string>
 *   Diagram.prompt(src, {target, reply, comments, include_resolved, instructions}) -> Promise<string>
 *   Diagram.apply(src, ops) -> Promise<new DSL source>
 *   Diagram.parse(src) -> Promise<{ok, document, errors}>
 *   Diagram.mount(element, {source, base, mode: "view"|"edit", ...DiagramEditor options}) -> Promise<editor>
 *   Diagram.renderAll(selector = "pre > code.language-diagram, .diagram") -> Promise
 *   Diagram.spec() / Diagram.schema() -> Promise<string>
 *
 * Declaratively: <div data-diagram data-mode="view" style="height:400px"><script type="text/plain">a -> b</script></div>
 */
(function (root) {
  "use strict";
  var ready = null;
  var cssDone = false;

  function injectCss() {
    if (cssDone || typeof DIAGRAM_CSS === "undefined" || !DIAGRAM_CSS || typeof document === "undefined") return;
    var s = document.createElement("style");
    s.setAttribute("data-diagram-css", "");
    s.textContent = DIAGRAM_CSS;
    document.head.appendChild(s);
    cssDone = true;
  }

  function b64ToBytes(b64) {
    var bin = atob(b64), out = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  function wasm() {
    if (!ready) {
      if (typeof DIAGRAM_WASM_B64 !== "undefined" && DIAGRAM_WASM_B64) ready = DIAGRAM_WASM_INIT(b64ToBytes(DIAGRAM_WASM_B64));
      else if (root.wasm_bindgen) ready = root.wasm_bindgen({ module_or_path: (root.DIAGRAM_WASM_URL || "pkg/diagram_wasm_bg.wasm") }).then(function () { return root.wasm_bindgen; });
      else ready = Promise.reject(new Error("diagram: no WASM module available"));
    }
    return ready;
  }

  function withDoc(src, fn) {
    return wasm().then(function (w) {
      var d = new w.Diagram();
      try {
        var res = JSON.parse(d.set_source(src));
        if (!res.ok) throw Object.assign(new Error(res.errors.map(function (e) { return (e.line ? "line " + e.line + ": " : "") + e.message; }).join("\n")), { errors: res.errors });
        return fn(d, w);
      } finally {
        d.free();
      }
    });
  }

  function textOf(el) {
    var s = el.querySelector('script[type="text/plain"], script[type="text/x-diagram"], script[type="application/json"]');
    return (s ? s.textContent : el.textContent).replace(/^\n/, "");
  }

  var Diagram = {
    version: function () { return wasm().then(function (w) { return w.version(); }); },
    ready: function () { return wasm().then(function () { return Diagram; }); },
    render: function (src, opts) {
      return withDoc(src, function (d) {
        var r = JSON.parse(d.render(JSON.stringify(opts || {})));
        if (r.error) throw new Error(r.error);
        return r.svg;
      });
    },
    layout: function (src, opts) { return withDoc(src, function (d) { return JSON.parse(d.render(JSON.stringify(opts || {}))); }); },
    convert: function (src, to) { return withDoc(src, function (d) { return d.source(to || "dsl"); }); },
    prompt: function (src, opts) { return withDoc(src, function (d) { return d.prompt(JSON.stringify(opts || {})); }); },
    brief: function (src) { return withDoc(src, function (d) { return d.brief(); }); },
    apply: function (src, ops) {
      return withDoc(src, function (d) {
        var res = JSON.parse(d.apply(typeof ops === "string" ? ops : JSON.stringify(Array.isArray(ops) ? { ops: ops } : ops)));
        if (!res.ok) throw new Error(res.error);
        return d.source("dsl");
      });
    },
    parse: function (src) {
      return wasm().then(function (w) {
        var d = new w.Diagram();
        try {
          var res = JSON.parse(d.set_source(src));
          return { ok: res.ok, format: res.format, errors: res.errors, document: JSON.parse(d.document()) };
        } finally { d.free(); }
      });
    },
    spec: function () { return wasm().then(function (w) { return w.spec(); }); },
    schema: function () { return wasm().then(function (w) { return JSON.parse(w.json_schema()); }); },
    mount: function (el, opts) {
      opts = opts || {};
      injectCss();
      if (typeof el === "string") el = document.querySelector(el);
      var source = opts.source != null ? opts.source : textOf(el);
      return wasm().then(function (w) {
        return root.DiagramEditor.create(el, Object.assign({}, opts, { wasm: w, source: source }));
      });
    },
    /* Replace code blocks (e.g. Markdown ```diagram fences) with rendered SVGs. */
    renderAll: function (selector, opts) {
      selector = selector || "pre > code.language-diagram, .diagram";
      var els = Array.prototype.slice.call(document.querySelectorAll(selector));
      return Promise.all(els.map(function (el) {
        var src = el.textContent;
        var target = el.parentElement && el.parentElement.tagName === "PRE" ? el.parentElement : el;
        return Diagram.render(src, opts).then(function (svg) {
          var fig = document.createElement("figure");
          fig.className = "diagram-figure";
          fig.innerHTML = svg;
          var s = fig.querySelector("svg");
          s.style.maxWidth = "100%";
          s.style.height = "auto";
          target.replaceWith(fig);
        }, function (e) {
          target.setAttribute("title", e.message);
          target.style.outline = "2px solid #cf222e";
        });
      }));
    }
  };

  root.Diagram = Diagram;

  // <div data-diagram> elements mount themselves
  if (typeof document !== "undefined") {
    var auto = function () {
      document.querySelectorAll("div[data-diagram]:not([data-diagram-mounted])").forEach(function (el) {
        el.setAttribute("data-diagram-mounted", "");
        Diagram.mount(el, { mode: el.getAttribute("data-mode") || "view", theme: el.getAttribute("data-theme") || null });
      });
    };
    if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", auto);
    else auto();
  }
})(typeof window !== "undefined" ? window : this);
