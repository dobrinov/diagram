/* diagram editor component.
 *
 *   var ed = DiagramEditor.create(element, {
 *     wasm,               // the wasm_bindgen namespace (has `Diagram`)
 *     source,             // DSL, JSON or Mermaid
 *     mode: "edit",       // or "view" (pan, zoom, comments, export only)
 *     theme: null,        // "light" | "dark" | null (follow the OS)
 *     author: "deyan",    // name used for new comments
 *     code: true,         // show the code pane (edit mode)
 *     panel: true,        // show the inspector / comments panel
 *     shareUrl: null,     // playground URL, enables "Copy share link"
 *     onChange: function (source, ed) {},
 *     onSave: function (source, ed) {}   // Cmd/Ctrl+S
 *   });
 *
 *   ed.getSource(fmt)  ed.setSource(src)  ed.apply(ops)  ed.getDocument()
 *   ed.setBase(src|null)  ed.prompt(opts)  ed.brief()  ed.exportSVG()  ed.exportPNG(scale)
 *   ed.select(ids)  ed.fit()  ed.setTheme(t)  ed.addButton({label, title, onClick})  ed.on(evt, fn)  ed.destroy()
 *
 * Everything that changes the diagram goes through the core's ops API, so the
 * code pane, the canvas, undo and LLM replies all stay in step.
 */
(function (root) {
  "use strict";

  var SHAPES = ["box", "rounded", "pill", "circle", "diamond", "hexagon", "cylinder", "parallelogram", "document", "person", "cloud", "sticky", "text"];
  var COLORS = ["gray", "blue", "green", "yellow", "orange", "red", "purple", "pink", "teal"];
  var SWATCH = { gray: "#868e96", blue: "#3b6fd8", green: "#2f9e44", yellow: "#e0b000", orange: "#e8590c", red: "#e03131", purple: "#7048e8", pink: "#d6336c", teal: "#0c8599" };
  var MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  var MOD = MAC ? "⌘" : "Ctrl+";

  var ICON = {
    select: '<path d="M5 3l14 8-6 1.5L10 19z"/>',
    node: '<rect x="4" y="6" width="16" height="12" rx="2"/>',
    sticky: '<path d="M5 4h14v10l-5 6H5z"/><path d="M14 20v-6h5"/>',
    connect: '<path d="M5 19L19 5"/><path d="M11 5h8v8"/>',
    comment: '<path d="M4 5h16v11H9l-5 4z"/>',
    undo: '<path d="M9 7L4 12l5 5"/><path d="M4 12h11a5 5 0 010 10h-3"/>',
    redo: '<path d="M15 7l5 5-5 5"/><path d="M20 12H9a5 5 0 000 10h3"/>',
    code: '<path d="M8 7l-5 5 5 5"/><path d="M16 7l5 5-5 5"/>',
    panel: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>',
    theme: '<circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 000 16z" fill="currentColor"/>',
    tidy: '<path d="M4 6h7M4 12h11M4 18h16"/>',
    fit: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>'
  };
  function icon(name) {
    return '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">' + ICON[name] + "</svg>";
  }

  function h(tag, attrs, kids) {
    var el = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) {
      var v = attrs[k];
      if (v == null || v === false) return;
      if (k === "class") el.className = v;
      else if (k === "html") el.innerHTML = v;
      else if (k === "text") el.textContent = v;
      else if (k.slice(0, 2) === "on") el.addEventListener(k.slice(2), v);
      else el.setAttribute(k, v === true ? "" : v);
    });
    (kids || []).forEach(function (c) { if (c != null) el.appendChild(typeof c === "string" ? document.createTextNode(c) : c); });
    return el;
  }

  function slug(s) {
    var out = String(s).toLowerCase().normalize("NFKD").replace(/[\u0300-\u036f]/g, "").replace(/[^\p{L}\p{N}_]+/gu, "-").replace(/^-+|-+$/g, "");
    return out || "node";
  }

  function download(name, data, type) {
    var blob = data instanceof Blob ? data : new Blob([data], { type: type || "text/plain" });
    var a = h("a", { href: URL.createObjectURL(blob), download: name });
    document.body.appendChild(a);
    a.click();
    setTimeout(function () { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) return navigator.clipboard.writeText(text);
    var ta = h("textarea", { style: "position:fixed;opacity:0" });
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); } finally { ta.remove(); }
    return Promise.resolve();
  }

  function b64url(bytes) {
    var s = "";
    for (var i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }

  /* Encode a diagram for a URL hash: deflate when the browser can. */
  function encodeShare(text) {
    var bytes = new TextEncoder().encode(text);
    if (typeof CompressionStream === "undefined") return Promise.resolve("t=" + b64url(bytes));
    var stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream("deflate-raw"));
    return new Response(stream).arrayBuffer().then(function (buf) { return "z=" + b64url(new Uint8Array(buf)); });
  }

  function decodeShare(hash) {
    var m = /(?:^|[#&])([zt])=([A-Za-z0-9_-]+)/.exec(hash || "");
    if (!m) return Promise.resolve(null);
    var bin = atob(m[2].replace(/-/g, "+").replace(/_/g, "/"));
    var bytes = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    if (m[1] === "t") return Promise.resolve(new TextDecoder().decode(bytes));
    var stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
    return new Response(stream).text();
  }

  function create(el, opts) {
    opts = opts || {};
    if (!opts.wasm || !opts.wasm.Diagram) throw new Error("DiagramEditor: pass the initialised wasm module as `wasm`");
    var wasm = opts.wasm;
    var view = opts.mode === "view";
    var listeners = {};
    var st = {
      session: new wasm.Diagram(),
      r: null, doc: null, text: "", errors: [],
      zoom: 1, panX: 0, panY: 0, fitted: false,
      tool: "select", sel: [], hover: null, comment: null, composer: null,
      undo: [], redo: [], codeFormat: "dsl",
      dark: opts.theme ? opts.theme === "dark" : !!(root.matchMedia && root.matchMedia("(prefers-color-scheme: dark)").matches),
      showResolved: false, base: null, space: false,
      author: opts.author || lsGet("diagram.author") || ""
    };

    function lsGet(k) { try { return localStorage.getItem(k); } catch (e) { return null; } }
    function lsSet(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* private mode */ } }
    function emit(name, arg) { (listeners[name] || []).forEach(function (fn) { fn(arg, api); }); }

    // ------------------------------------------------------------- layout
    el.innerHTML = "";
    el.classList.add("dge");
    if (view) el.classList.add("dge-view");
    var toolbar = h("div", { class: "dge-toolbar", role: "toolbar" });
    var codeTabs = h("div", { class: "dge-tabs" });
    var gutter = h("div", { class: "dge-gutter", "aria-hidden": "true" });
    var textarea = h("textarea", { class: "dge-text", spellcheck: "false", autocapitalize: "off", autocomplete: "off", "aria-label": "Diagram source" });
    var errorsEl = h("div", { class: "dge-errors" });
    var codePane = h("div", { class: "dge-code" }, [codeTabs, h("div", { class: "dge-code-wrap" }, [gutter, textarea]), errorsEl]);
    var stage = h("div", { class: "dge-stage" });
    var overlay = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    overlay.setAttribute("class", "dge-overlay");
    var zoomLabel = h("button", { class: "dge-zoom-label", title: "Reset zoom (0)", onclick: function () { zoomTo(1); } }, ["100%"]);
    var zoomBar = h("div", { class: "dge-zoombar" }, [
      h("button", { title: "Zoom out (-)", onclick: function () { zoomTo(st.zoom / 1.2); } }, ["−"]),
      zoomLabel,
      h("button", { title: "Zoom in (+)", onclick: function () { zoomTo(st.zoom * 1.2); } }, ["+"]),
      h("button", { title: "Fit (F)", html: icon("fit"), onclick: function () { fit(); } })
    ]);
    var hint = h("div", { class: "dge-hint" });
    var canvas = h("div", { class: "dge-canvas", tabindex: "0" }, [stage, zoomBar, hint]);
    var inspector = h("div", { class: "dge-inspector" });
    var commentsEl = h("div", { class: "dge-comments" });
    var panel = h("aside", { class: "dge-panel" }, [inspector, commentsEl]);
    var body = h("div", { class: "dge-body" }, [codePane, canvas, panel]);
    var toast = h("div", { class: "dge-toast", role: "status" });
    var menu = h("div", { class: "dge-menu", hidden: true });
    var modal = h("div", { class: "dge-modal", hidden: true });
    el.appendChild(toolbar);
    el.appendChild(body);
    el.appendChild(toast);
    el.appendChild(menu);
    el.appendChild(modal);

    var narrow = el.clientWidth && el.clientWidth < 760;
    var showCode = !view && opts.code !== false && !narrow;
    var showPanel = opts.panel !== false && !narrow && !view;

    function applyChrome() {
      el.setAttribute("data-theme", st.dark ? "dark" : "light");
      codePane.hidden = !showCode;
      panel.hidden = !showPanel;
      toolbar.querySelectorAll("[data-tool]").forEach(function (b) { b.classList.toggle("on", b.getAttribute("data-tool") === st.tool); });
      var cb = toolbar.querySelector("[data-act=code]");
      if (cb) cb.classList.toggle("on", showCode);
      var pb = toolbar.querySelector("[data-act=panel]");
      if (pb) pb.classList.toggle("on", showPanel);
      canvas.setAttribute("data-tool", st.tool);
    }

    function btn(attrs, label) {
      var b = h("button", attrs);
      b.innerHTML = label;
      return b;
    }

    function buildToolbar() {
      toolbar.innerHTML = "";
      if (!view) {
        var tools = h("div", { class: "dge-group" });
        [["select", "Select & move (V)"], ["node", "Add a box (N)"], ["sticky", "Add a sticky note (S)"], ["connect", "Connect (C) — or drag a node's handle"], ["comment", "Comment (M)"]].forEach(function (t) {
          tools.appendChild(btn({ "data-tool": t[0], title: t[1], onclick: function () { setTool(t[0]); } }, icon(t[0])));
        });
        toolbar.appendChild(tools);
        toolbar.appendChild(h("div", { class: "dge-group" }, [
          btn({ title: "Undo (" + MOD + "Z)", onclick: undo }, icon("undo")),
          btn({ title: "Redo (" + MOD + "⇧Z)", onclick: redo }, icon("redo"))
        ]));
        var dir = h("select", { title: "Direction", class: "dge-select", onchange: function (e) { applyOps([{ op: "set", key: "direction", value: e.target.value }]); } });
        [["right", "→ Right"], ["down", "↓ Down"], ["left", "← Left"], ["up", "↑ Up"]].forEach(function (d) { dir.appendChild(h("option", { value: d[0] }, [d[1]])); });
        var routing = h("select", { title: "Edge style", class: "dge-select", onchange: function (e) { applyOps([{ op: "set", key: "routing", value: e.target.value }]); } });
        [["orthogonal", "Elbow"], ["curved", "Curved"], ["straight", "Straight"]].forEach(function (d) { routing.appendChild(h("option", { value: d[0] }, [d[1]])); });
        toolbar.appendChild(h("div", { class: "dge-group" }, [dir, routing, btn({ title: "Tidy up: let the layout place every node again", onclick: function () { applyOps([{ op: "unpin" }]); fitSoon(); } }, icon("tidy"))]));
        toolbar.dir = dir;
        toolbar.routing = routing;
      }
      toolbar.appendChild(h("div", { class: "dge-spacer" }));
      toolbar.extra = h("div", { class: "dge-group dge-extra" });
      toolbar.appendChild(toolbar.extra);
      (st.extraButtons || []).forEach(function (b) { toolbar.extra.appendChild(b); });
      toolbar.baseChip = h("span", { class: "dge-chip", hidden: true });
      toolbar.appendChild(toolbar.baseChip);
      var right = h("div", { class: "dge-group" });
      right.appendChild(h("button", { class: "dge-primary", title: "Turn open comments into a prompt for an LLM", onclick: function () { openPrompt(); } }, ["Prompt"]));
      right.appendChild(h("button", { title: "Export and share", onclick: function (e) { openExport(e.currentTarget); } }, ["Export ▾"]));
      if (!view && opts.code !== false) right.appendChild(btn({ "data-act": "code", title: "Code (" + MOD + "/)", onclick: function () { showCode = !showCode; applyChrome(); } }, icon("code")));
      right.appendChild(btn({ "data-act": "panel", title: "Inspector & comments", onclick: function () { showPanel = !showPanel; applyChrome(); renderPanel(); } }, icon("panel")));
      right.appendChild(btn({ title: "Light / dark", onclick: function () { st.dark = !st.dark; applyChrome(); render(); } }, icon("theme")));
      toolbar.appendChild(right);
    }

    function buildTabs() {
      codeTabs.innerHTML = "";
      [["dsl", "Diagram"], ["json", "JSON"], ["mermaid", "Mermaid"]].forEach(function (t) {
        codeTabs.appendChild(h("button", { class: st.codeFormat === t[0] ? "on" : "", onclick: function () { st.codeFormat = t[0]; buildTabs(); syncCode(true); } }, [t[1]]));
      });
      codeTabs.appendChild(h("a", { class: "dge-help", href: "https://github.com/dobrinov/diagram/blob/main/crates/core/LANGUAGE.md", target: "_blank", rel: "noopener" }, ["syntax ↗"]));
    }

    // --------------------------------------------------------- doc state
    function refreshDoc() {
      st.doc = JSON.parse(st.session.document());
    }
    function node(id) { return st.doc && st.doc.nodes.find(function (n) { return n.id === id; }); }
    function edge(id) { return st.doc && (st.doc.edges || []).find(function (e) { return e.id === id; }); }
    function group(id) { return st.doc && (st.doc.groups || []).find(function (g) { return g.id === id; }); }
    function comment(id) { return st.doc && (st.doc.comments || []).find(function (c) { return c.id === id; }); }
    function kindOf(id) { return node(id) ? "node" : edge(id) ? "edge" : group(id) ? "group" : null; }
    function idTaken(id) { return !!(node(id) || edge(id) || group(id)); }
    function freshId(base) {
      if (!idTaken(base)) return base;
      for (var i = 2; ; i++) if (!idTaken(base + "-" + i)) return base + "-" + i;
    }
    function labelOf(id) {
      var n = node(id) || group(id);
      if (n) return n.label || n.id;
      var e = edge(id);
      if (e) return (e.label ? "“" + e.label + "” " : "") + e.from + " → " + e.to;
      return id;
    }

    function load(src, record) {
      if (record && st.text !== src) pushUndo();
      var res = JSON.parse(st.session.set_source(src));
      st.errors = res.errors || [];
      st.text = res.format === "dsl" ? src : st.session.source("dsl");
      refreshDoc();
      return res;
    }

    function pushUndo() {
      st.undo.push(st.text);
      if (st.undo.length > 200) st.undo.shift();
      st.redo = [];
    }
    function undo() {
      if (!st.undo.length) return;
      st.redo.push(st.text);
      load(st.undo.pop(), false);
      afterChange(true);
    }
    function redo() {
      if (!st.redo.length) return;
      st.undo.push(st.text);
      load(st.redo.pop(), false);
      afterChange(true);
    }

    function afterChange(syncText) {
      if (syncText) syncCode(true);
      showErrors();
      render();
      emit("change", st.text);
      if (opts.onChange) opts.onChange(st.text, api);
    }

    /* Apply ops through the core. `live` skips undo/code sync (used while dragging). */
    function applyOps(ops, live) {
      var before = st.text;
      var res = JSON.parse(st.session.apply(JSON.stringify({ ops: ops })));
      if (!res.ok) {
        flash(res.error || "could not apply the change", true);
        return res;
      }
      refreshDoc();
      st.errors = [];
      if (live) {
        render();
        return res;
      }
      st.undo.push(before);
      if (st.undo.length > 200) st.undo.shift();
      st.redo = [];
      st.text = st.session.source("dsl");
      afterChange(true);
      return res;
    }

    // ------------------------------------------------------------ code pane
    function syncCode(force) {
      if (!showCode && !force) return;
      var text = st.codeFormat === "dsl" ? st.text : st.session.source(st.codeFormat);
      if (textarea.value !== text) {
        var top = textarea.scrollTop;
        textarea.value = text;
        textarea.scrollTop = top;
      }
      textarea.readOnly = st.codeFormat === "mermaid";
      updateGutter();
    }
    function updateGutter() {
      var lines = textarea.value.split("\n").length;
      var bad = {};
      if (st.codeFormat === "dsl") st.errors.forEach(function (e) { bad[e.line] = e.message; });
      var html = "";
      for (var i = 1; i <= lines; i++) html += '<div class="' + (bad[i] ? "err" : "") + '"' + (bad[i] ? ' title="' + bad[i].replace(/"/g, "&quot;") + '"' : "") + ">" + i + "</div>";
      gutter.innerHTML = html;
      gutter.scrollTop = textarea.scrollTop;
    }
    function showErrors() {
      errorsEl.innerHTML = "";
      st.errors.forEach(function (e) {
        errorsEl.appendChild(h("button", { class: "dge-error", onclick: function () { gotoLine(e.line); } }, [(e.line ? "line " + e.line + ": " : "") + e.message]));
      });
      errorsEl.hidden = !st.errors.length;
      updateGutter();
    }
    function gotoLine(line) {
      if (!line) return;
      var lines = textarea.value.split("\n");
      var start = 0;
      for (var i = 0; i < line - 1 && i < lines.length; i++) start += lines[i].length + 1;
      textarea.focus();
      textarea.setSelectionRange(start, start + (lines[line - 1] || "").length);
      textarea.scrollTop = Math.max(0, (line - 5) * 20);
    }
    var typing = null;
    textarea.addEventListener("input", function () {
      updateGutter();
      clearTimeout(typing);
      typing = setTimeout(function () {
        if (st.codeFormat === "mermaid") return;
        var before = st.text;
        var res = JSON.parse(st.session.set_source(textarea.value));
        st.errors = res.errors || [];
        if (st.text !== textarea.value) {
          st.undo.push(before);
          st.redo = [];
        }
        st.text = st.codeFormat === "dsl" ? textarea.value : st.session.source("dsl");
        refreshDoc();
        afterChange(false);
      }, 180);
    });
    textarea.addEventListener("scroll", function () { gutter.scrollTop = textarea.scrollTop; });
    textarea.addEventListener("keydown", function (e) {
      if (e.key === "Tab") {
        e.preventDefault();
        document.execCommand("insertText", false, "  ");
      }
    });

    // --------------------------------------------------------------- render
    function render() {
      var r = JSON.parse(st.session.render(JSON.stringify({ comments: st.showResolved ? "all" : "open", theme: st.dark ? "dark" : "light" })));
      if (r.error) {
        flash(r.error, true);
        return;
      }
      var old = st.r;
      st.r = r;
      stage.innerHTML = r.svg;
      overlay.setAttribute("viewBox", [r.view.x, r.view.y, r.view.w, r.view.h].join(" "));
      overlay.setAttribute("width", r.view.w);
      overlay.setAttribute("height", r.view.h);
      stage.appendChild(overlay);
      if (old && st.fitted) {
        st.panX += (r.view.x - old.view.x) * st.zoom;
        st.panY += (r.view.y - old.view.y) * st.zoom;
      }
      if (!st.fitted && canvas.clientWidth) {
        fit();
        st.fitted = true;
      }
      transform();
      markSelection();
      drawOverlay();
      renderPanel();
      if (toolbar.dir && st.doc) {
        var s = st.doc.settings || {};
        toolbar.dir.value = s.direction || "right";
        toolbar.routing.value = s.routing || "orthogonal";
      }
      var nodes = st.doc ? st.doc.nodes.length : 0;
      hint.textContent = nodes ? "" : view ? "Empty diagram" : "Double-click to add a box, or type in the code pane";
      hint.hidden = !!nodes;
    }

    function transform() {
      stage.style.transform = "translate(" + st.panX + "px," + st.panY + "px) scale(" + st.zoom + ")";
      zoomLabel.textContent = Math.round(st.zoom * 100) + "%";
    }
    function fit() {
      if (!st.r) return;
      var cw = canvas.clientWidth, ch = canvas.clientHeight;
      if (!cw || !ch) return;
      var v = st.r.view;
      st.zoom = Math.max(0.1, Math.min(1.5, Math.min(cw / v.w, ch / v.h) * 0.94));
      st.panX = (cw - v.w * st.zoom) / 2;
      st.panY = (ch - v.h * st.zoom) / 2;
      transform();
    }
    function fitSoon() { setTimeout(fit, 0); }
    function zoomTo(z, cx, cy) {
      z = Math.max(0.1, Math.min(4, z));
      if (cx == null) { cx = canvas.clientWidth / 2; cy = canvas.clientHeight / 2; }
      st.panX = cx - (cx - st.panX) * (z / st.zoom);
      st.panY = cy - (cy - st.panY) * (z / st.zoom);
      st.zoom = z;
      transform();
    }
    function toDiagram(clientX, clientY) {
      var rect = canvas.getBoundingClientRect();
      return { x: st.r.view.x + (clientX - rect.left - st.panX) / st.zoom, y: st.r.view.y + (clientY - rect.top - st.panY) / st.zoom };
    }
    function toScreen(x, y) {
      return { x: st.panX + (x - st.r.view.x) * st.zoom, y: st.panY + (y - st.r.view.y) * st.zoom };
    }
    function geom(id) {
      if (!st.r) return null;
      return st.r.nodes.find(function (n) { return n.id === id; }) || st.r.groups.find(function (g) { return g.id === id; });
    }

    function markSelection() {
      stage.querySelectorAll(".sel").forEach(function (e) { e.classList.remove("sel"); });
      st.sel.forEach(function (id) {
        stage.querySelectorAll('[data-id="' + cssEsc(id) + '"]').forEach(function (e) { e.classList.add("sel"); });
      });
      if (st.comment) stage.querySelectorAll('[data-comment="' + cssEsc(st.comment) + '"]').forEach(function (e) { e.classList.add("sel"); });
      if (st.comment) {
        var c = comment(st.comment);
        if (c && c.on) stage.querySelectorAll('[data-id="' + cssEsc(c.on) + '"]').forEach(function (e) { e.classList.add("cmt"); });
      }
    }
    function cssEsc(s) { return String(s).replace(/["\\]/g, "\\$&"); }

    var SVGNS = "http://www.w3.org/2000/svg";
    function svgEl(tag, attrs) {
      var e = document.createElementNS(SVGNS, tag);
      Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
      return e;
    }
    function drawOverlay() {
      while (overlay.firstChild) overlay.removeChild(overlay.firstChild);
      if (view || !st.r) return;
      var targets = st.drag && st.drag.type === "connect" ? [] : st.sel.filter(function (id) { return node(id); });
      if (st.hover && node(st.hover) && targets.indexOf(st.hover) < 0) targets.push(st.hover);
      if (st.tool !== "select" && st.tool !== "connect") targets = [];
      targets.forEach(function (id) {
        var g = geom(id);
        if (!g) return;
        var r = 5 / Math.max(st.zoom, 0.4);
        [[g.x + g.w / 2, g.y], [g.x + g.w, g.y + g.h / 2], [g.x + g.w / 2, g.y + g.h], [g.x, g.y + g.h / 2]].forEach(function (p) {
          var c = svgEl("circle", { cx: p[0], cy: p[1], r: r, class: "dge-handle", "data-node": id });
          overlay.appendChild(c);
        });
      });
      var d = st.drag;
      if (d && d.type === "connect" && d.to) {
        var g0 = geom(d.from);
        overlay.appendChild(svgEl("path", { class: "dge-rubber", d: "M" + (g0.x + g0.w / 2) + "," + (g0.y + g0.h / 2) + " L" + d.to.x + "," + d.to.y }));
        if (d.target) {
          var gt = geom(d.target);
          overlay.appendChild(svgEl("rect", { class: "dge-target", x: gt.x - 4, y: gt.y - 4, width: gt.w + 8, height: gt.h + 8, rx: 8 }));
        }
      }
      if (d && d.type === "marquee" && d.rect) {
        overlay.appendChild(svgEl("rect", { class: "dge-marquee", x: d.rect.x, y: d.rect.y, width: d.rect.w, height: d.rect.h }));
      }
    }

    // ---------------------------------------------------------- interaction
    function setTool(t) {
      if (view) t = "select";
      st.tool = t;
      st.drag = null;
      applyChrome();
      drawOverlay();
      var tips = { node: "Click to place a box", sticky: "Click to place a sticky note", connect: "Drag from one node to another (or to empty space)", comment: "Click a node, edge, group or the canvas to comment" };
      if (tips[t]) flash(tips[t]);
    }

    function select(ids, additive) {
      if (additive) {
        ids.forEach(function (id) {
          var i = st.sel.indexOf(id);
          if (i >= 0) st.sel.splice(i, 1);
          else st.sel.push(id);
        });
      } else st.sel = ids.slice();
      st.comment = null;
      markSelection();
      drawOverlay();
      renderPanel();
      emit("select", st.sel.slice());
    }

    function hit(target) {
      if (!target || !target.closest) return null;
      var h1 = target.closest(".dge-handle");
      if (h1) return { type: "handle", id: h1.getAttribute("data-node") };
      var pin = target.closest(".dg-pin");
      if (pin) return { type: "pin", id: pin.getAttribute("data-comment") };
      var n = target.closest(".dg-node");
      if (n) return { type: "node", id: n.getAttribute("data-id") };
      var e = target.closest(".dg-edge");
      if (e) return { type: "edge", id: e.getAttribute("data-id") };
      var g = target.closest(".dg-group");
      if (g) return { type: "group", id: g.getAttribute("data-id") };
      return null;
    }

    function membersOf(gid) {
      var out = [];
      var groups = [gid];
      var changed = true;
      while (changed) {
        changed = false;
        (st.doc.groups || []).forEach(function (g) {
          if (g.parent && groups.indexOf(g.parent) >= 0 && groups.indexOf(g.id) < 0) { groups.push(g.id); changed = true; }
        });
      }
      st.doc.nodes.forEach(function (n) { if (n.group && groups.indexOf(n.group) >= 0) out.push(n.id); });
      return out;
    }

    function addNodeAt(p, shape, groupId, thenEdit) {
      var sticky = shape === "sticky";
      var w = sticky ? 150 : 120, hgt = sticky ? 110 : 44;
      var id = freshId(sticky ? "note" : "node");
      var op = { op: "add_node", id: id, label: sticky ? "Note" : "New box", x: Math.round(p.x - w / 2), y: Math.round(p.y - hgt / 2) };
      if (shape && shape !== "box") op.shape = shape;
      if (groupId) op.group = groupId;
      var res = applyOps([op]);
      if (res.ok) {
        select([id]);
        if (thenEdit) editLabel(id, true);
      }
      return res.ok ? id : null;
    }

    var raf = 0;
    function liveMove() {
      if (raf) return;
      raf = requestAnimationFrame(function () {
        raf = 0;
        var d = st.drag;
        if (!d || d.type !== "move") return;
        var ops = d.items.map(function (m) { return { op: "move", id: m.id, x: m.x + d.dx, y: m.y + d.dy }; });
        applyOps(ops, true);
      });
    }

    canvas.addEventListener("pointerdown", function (e) {
      if (!st.r) return;
      canvas.focus({ preventScroll: true });
      closeMenu();
      var p = toDiagram(e.clientX, e.clientY);
      var t = hit(e.target);
      if (e.button === 1 || st.space || (e.button === 0 && view && !(t && t.type === "pin"))) {
        st.drag = { type: "pan", sx: e.clientX, sy: e.clientY, px: st.panX, py: st.panY };
        canvas.setPointerCapture(e.pointerId);
        canvas.classList.add("panning");
        return;
      }
      if (e.button !== 0) return;
      if (t && t.type === "pin") {
        st.comment = t.id;
        st.sel = [];
        showPanel = true;
        applyChrome();
        markSelection();
        renderPanel();
        var card = commentsEl.querySelector('[data-cid="' + cssEsc(t.id) + '"]');
        if (card) card.scrollIntoView({ block: "nearest" });
        return;
      }
      if (st.tool === "comment") {
        st.composer = t && (t.type === "node" || t.type === "edge" || t.type === "group") ? { on: t.id } : { x: Math.round(p.x), y: Math.round(p.y) };
        showPanel = true;
        applyChrome();
        renderPanel();
        setTool("select");
        var ta = commentsEl.querySelector(".dge-composer textarea");
        if (ta) ta.focus();
        return;
      }
      if (st.tool === "node" || st.tool === "sticky") {
        var gid = t && t.type === "group" ? t.id : t && t.type === "node" ? (node(t.id) || {}).group : null;
        addNodeAt(p, st.tool === "sticky" ? "sticky" : "box", gid, true);
        setTool("select");
        return;
      }
      if (t && (t.type === "handle" || (st.tool === "connect" && t.type === "node"))) {
        st.drag = { type: "connect", from: t.id, to: p };
        canvas.setPointerCapture(e.pointerId);
        drawOverlay();
        return;
      }
      if (t && (t.type === "node" || t.type === "group" || t.type === "edge")) {
        if (e.shiftKey || e.metaKey) select([t.id], true);
        else if (st.sel.indexOf(t.id) < 0) select([t.id]);
        if (t.type === "edge") return;
        // everything selected moves; groups carry their members
        var ids = [];
        st.sel.forEach(function (id) {
          if (node(id)) ids.push(id);
          else if (group(id)) ids = ids.concat(membersOf(id));
        });
        var items = [];
        ids.forEach(function (id) {
          var g = geom(id);
          if (g && !items.some(function (i) { return i.id === id; })) items.push({ id: id, x: g.x, y: g.y });
        });
        st.drag = { type: "move", sx: p.x, sy: p.y, dx: 0, dy: 0, items: items, before: st.text, moved: false };
        canvas.setPointerCapture(e.pointerId);
        return;
      }
      if (!e.shiftKey) select([]);
      st.drag = { type: "marquee", sx: p.x, sy: p.y, additive: e.shiftKey };
      canvas.setPointerCapture(e.pointerId);
    });

    canvas.addEventListener("pointermove", function (e) {
      if (!st.r) return;
      var d = st.drag;
      if (!d) {
        var t = hit(e.target);
        var hov = t && (t.type === "node" || t.type === "handle") ? t.id : null;
        if (hov !== st.hover) {
          st.hover = hov;
          drawOverlay();
        }
        return;
      }
      if (d.type === "pan") {
        st.panX = d.px + e.clientX - d.sx;
        st.panY = d.py + e.clientY - d.sy;
        transform();
        return;
      }
      var p = toDiagram(e.clientX, e.clientY);
      if (d.type === "move") {
        d.dx = Math.round(p.x - d.sx);
        d.dy = Math.round(p.y - d.sy);
        if (!d.moved && Math.abs(d.dx) + Math.abs(d.dy) < 3) return;
        d.moved = true;
        liveMove();
      } else if (d.type === "connect") {
        d.to = p;
        var under = document.elementFromPoint(e.clientX, e.clientY);
        var t2 = hit(under);
        d.target = t2 && (t2.type === "node" || t2.type === "group" || t2.type === "handle") && t2.id !== d.from ? t2.id : null;
        drawOverlay();
      } else if (d.type === "marquee") {
        d.rect = { x: Math.min(d.sx, p.x), y: Math.min(d.sy, p.y), w: Math.abs(p.x - d.sx), h: Math.abs(p.y - d.sy) };
        drawOverlay();
      }
    });

    function endDrag(e) {
      var d = st.drag;
      st.drag = null;
      canvas.classList.remove("panning");
      if (!d) return;
      if (d.type === "move" && d.moved) {
        if (raf) { cancelAnimationFrame(raf); raf = 0; }
        var ops = d.items.map(function (m) { return { op: "move", id: m.id, x: m.x + d.dx, y: m.y + d.dy }; });
        st.session.set_source(d.before);
        st.text = d.before;
        applyOps(ops);
      } else if (d.type === "connect" && e) {
        var p = toDiagram(e.clientX, e.clientY);
        if (d.target) applyOps([{ op: "add_edge", from: d.from, to: d.target }]);
        else if (Math.hypot(p.x - (geom(d.from).x + geom(d.from).w / 2), p.y - (geom(d.from).y + geom(d.from).h / 2)) > 60) {
          // dropped on empty canvas: create a connected box there
          var before = st.text;
          var src = node(d.from);
          var id = freshId("node");
          var res = JSON.parse(st.session.apply(JSON.stringify({ ops: [
            { op: "add_node", id: id, label: "New box", x: Math.round(p.x - 60), y: Math.round(p.y - 22), group: src && src.group ? src.group : undefined },
            { op: "add_edge", from: d.from, to: id }
          ] })));
          if (res.ok) {
            st.undo.push(before);
            st.redo = [];
            refreshDoc();
            st.text = st.session.source("dsl");
            afterChange(true);
            select([id]);
            editLabel(id, true);
          } else flash(res.error, true);
        }
        drawOverlay();
      } else if (d.type === "marquee" && d.rect && (d.rect.w > 3 || d.rect.h > 3)) {
        var r = d.rect;
        var ids = st.r.nodes.filter(function (n) { return n.x < r.x + r.w && n.x + n.w > r.x && n.y < r.y + r.h && n.y + n.h > r.y; }).map(function (n) { return n.id; });
        select(ids, d.additive);
      } else drawOverlay();
    }
    canvas.addEventListener("pointerup", endDrag);
    canvas.addEventListener("pointercancel", function () { endDrag(null); });
    canvas.addEventListener("pointerleave", function () { if (st.hover && !st.drag) { st.hover = null; drawOverlay(); } });

    canvas.addEventListener("dblclick", function (e) {
      if (view || !st.r) return;
      var t = hit(e.target);
      if (t && (t.type === "node" || t.type === "edge" || t.type === "group")) {
        editLabel(t.id);
        return;
      }
      if (!t) addNodeAt(toDiagram(e.clientX, e.clientY), "box", null, true);
    });

    canvas.addEventListener("wheel", function (e) {
      e.preventDefault();
      var rect = canvas.getBoundingClientRect();
      if (e.ctrlKey || e.metaKey) {
        zoomTo(st.zoom * Math.exp(-e.deltaY * 0.01), e.clientX - rect.left, e.clientY - rect.top);
      } else {
        st.panX -= e.deltaX;
        st.panY -= e.deltaY;
        transform();
      }
    }, { passive: false });

    /* Inline label editor over a node, edge label or group title. */
    function editLabel(id, isNew) {
      var kind = kindOf(id);
      if (!kind || !st.r) return;
      var box, current;
      if (kind === "edge") {
        var ep = st.r.edges.find(function (x) { return x.id === id; });
        box = { x: ep.mid[0] - 70, y: ep.mid[1] - 14, w: 140, h: 28 };
        current = edge(id).label || "";
      } else if (kind === "group") {
        var gg = geom(id);
        box = { x: gg.x + 8, y: gg.y + 4, w: Math.max(160, gg.w * 0.6), h: 26 };
        current = group(id).label || id;
      } else {
        box = geom(id);
        current = node(id).label || id;
      }
      var tl = toScreen(box.x, box.y);
      var ta = h("textarea", { class: "dge-inline", spellcheck: "false" });
      ta.value = current;
      ta.style.left = tl.x + "px";
      ta.style.top = tl.y + "px";
      ta.style.width = Math.max(80, box.w * st.zoom) + "px";
      ta.style.height = Math.max(26, box.h * st.zoom) + "px";
      ta.style.fontSize = Math.max(11, 14 * st.zoom) + "px";
      canvas.appendChild(ta);
      ta.focus();
      ta.select();
      var done = false;
      function finish(save) {
        if (done) return;
        done = true;
        var v = ta.value.trim();
        ta.remove();
        canvas.focus({ preventScroll: true });
        if (!save || v === current) return;
        var ops;
        if (kind === "node") {
          ops = [{ op: "update_node", id: id, label: v }];
          var nice = slug(v);
          // a box that was just added gets an id from its label
          if (isNew && v && nice !== id && !idTaken(nice)) ops.push({ op: "rename", id: id, to: nice });
          else if (isNew && v && nice !== id && !/^(node|note)(-\d+)?$/.test(nice)) ops.push({ op: "rename", id: id, to: freshId(nice) });
        } else if (kind === "edge") ops = [{ op: "update_edge", id: id, label: v }];
        else ops = [{ op: "update_group", id: id, label: v }];
        var res = applyOps(ops);
        if (res.ok && ops.length > 1) select([ops[1].to]);
      }
      ta.addEventListener("keydown", function (e) {
        e.stopPropagation();
        if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); finish(true); }
        if (e.key === "Escape") { e.preventDefault(); finish(false); }
      });
      ta.addEventListener("blur", function () { finish(true); });
    }

    function deleteSelection() {
      if (!st.sel.length) return;
      applyOps(st.sel.filter(kindOf).map(function (id) { return { op: "remove", id: id }; }));
      select([]);
    }

    function groupSelection() {
      var members = st.sel.filter(function (id) { return node(id) || group(id); });
      if (!members.length) return;
      var id = freshId("group");
      var res = applyOps([{ op: "add_group", id: id, label: "Group", members: members }]);
      if (res.ok) {
        select([id]);
        editLabel(id, false);
      }
    }

    function duplicateSelection() {
      var ops = [], ids = [];
      st.sel.forEach(function (id) {
        var n = node(id), g = geom(id);
        if (!n || !g) return;
        var nid = freshId(n.id + "-copy");
        while (ids.indexOf(nid) >= 0) nid = nid + "-x";
        ids.push(nid);
        ops.push({ op: "add_node", id: nid, label: n.label || n.id, shape: n.shape, color: n.color, detail: n.detail, group: n.group, x: Math.round(g.x + 24), y: Math.round(g.y + 24) });
      });
      if (ops.length && applyOps(ops).ok) select(ids);
    }

    function nudge(dx, dy) {
      var ops = [];
      st.sel.forEach(function (id) {
        var g = geom(id);
        if (node(id) && g) ops.push({ op: "move", id: id, x: g.x + dx, y: g.y + dy });
      });
      if (ops.length) applyOps(ops);
    }

    function onKey(e) {
      var tag = (e.target && e.target.tagName) || "";
      var typing = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (e.target && e.target.isContentEditable);
      var mod = e.metaKey || e.ctrlKey;
      if (!modal.hidden) {
        if (e.key === "Escape") closeModal();
        return;
      }
      if (mod && e.key.toLowerCase() === "s") {
        if (opts.onSave) { e.preventDefault(); opts.onSave(st.text, api); }
        return;
      }
      if (mod && e.key === "/") { e.preventDefault(); showCode = !showCode; applyChrome(); syncCode(true); return; }
      if (typing || !el.contains(document.activeElement) && document.activeElement !== document.body) return;
      if (e.key === " " && !st.space) { st.space = true; canvas.classList.add("space"); e.preventDefault(); return; }
      if (mod && e.key.toLowerCase() === "z") { e.preventDefault(); if (e.shiftKey) redo(); else undo(); return; }
      if (mod && e.key.toLowerCase() === "y") { e.preventDefault(); redo(); return; }
      if (view) {
        if (e.key === "f") fit();
        return;
      }
      if (mod && e.key.toLowerCase() === "a") { e.preventDefault(); select(st.doc.nodes.map(function (n) { return n.id; })); return; }
      if (mod && e.key.toLowerCase() === "d") { e.preventDefault(); duplicateSelection(); return; }
      if (mod && e.key.toLowerCase() === "g") { e.preventDefault(); groupSelection(); return; }
      if (mod) return;
      switch (e.key) {
        case "v": case "V": setTool("select"); break;
        case "n": case "N": case "r": setTool("node"); break;
        case "s": case "S": setTool("sticky"); break;
        case "c": case "C": setTool("connect"); break;
        case "m": case "M": setTool("comment"); break;
        case "f": fit(); break;
        case "0": zoomTo(1); break;
        case "+": case "=": zoomTo(st.zoom * 1.2); break;
        case "-": zoomTo(st.zoom / 1.2); break;
        case "Enter": if (st.sel.length === 1) { e.preventDefault(); editLabel(st.sel[0]); } break;
        case "Delete": case "Backspace": e.preventDefault(); deleteSelection(); break;
        case "Escape": if (st.tool !== "select") setTool("select"); else select([]); closeMenu(); break;
        case "ArrowLeft": e.preventDefault(); nudge(e.shiftKey ? -40 : -8, 0); break;
        case "ArrowRight": e.preventDefault(); nudge(e.shiftKey ? 40 : 8, 0); break;
        case "ArrowUp": e.preventDefault(); nudge(0, e.shiftKey ? -40 : -8); break;
        case "ArrowDown": e.preventDefault(); nudge(0, e.shiftKey ? 40 : 8); break;
        case "?": flash("V select · N box · S sticky · C connect · M comment · double-click to add/edit · " + MOD + "G group · " + MOD + "D duplicate · F fit · space+drag pan", false, 6000); break;
        default: return;
      }
    }
    function onKeyUp(e) {
      if (e.key === " ") { st.space = false; canvas.classList.remove("space"); }
    }
    document.addEventListener("keydown", onKey);
    document.addEventListener("keyup", onKeyUp);

    // ---------------------------------------------------------------- panel
    function field(label, input) {
      return h("label", { class: "dge-field" }, [h("span", {}, [label]), input]);
    }
    function textInput(value, onCommit, attrs) {
      var i = h("input", Object.assign({ type: "text", value: value || "" }, attrs || {}));
      i.addEventListener("change", function () { onCommit(i.value); });
      i.addEventListener("keydown", function (e) { if (e.key === "Enter") i.blur(); e.stopPropagation(); });
      return i;
    }
    function selectInput(options, value, onChange) {
      var s = h("select", { class: "dge-select" });
      options.forEach(function (o) {
        var v = Array.isArray(o) ? o[0] : o, l = Array.isArray(o) ? o[1] : o;
        var opt = h("option", { value: v }, [l]);
        if (v === (value || "")) opt.selected = true;
        s.appendChild(opt);
      });
      s.addEventListener("change", function () { onChange(s.value); });
      return s;
    }
    function swatches(value, onPick) {
      var row = h("div", { class: "dge-swatches" });
      row.appendChild(h("button", { class: "dge-swatch none" + (!value ? " on" : ""), title: "Default", onclick: function () { onPick(""); } }));
      COLORS.forEach(function (c) {
        row.appendChild(h("button", { class: "dge-swatch" + (value === c ? " on" : ""), title: c, style: "background:" + SWATCH[c], onclick: function () { onPick(c); } }));
      });
      return row;
    }

    function renderInspector() {
      inspector.innerHTML = "";
      if (!st.doc) return;
      var sel = st.sel.filter(kindOf);
      if (view) {
        var d0 = st.doc;
        inspector.appendChild(h("h3", {}, [d0.title || "Diagram"]));
        if (d0.description) inspector.appendChild(h("p", { class: "dge-muted" }, [d0.description]));
        return;
      }
      if (!sel.length) {
        var d = st.doc, s = d.settings || {};
        inspector.appendChild(h("h3", {}, ["Diagram"]));
        inspector.appendChild(field("Title", textInput(d.title, function (v) { applyOps([{ op: "set", key: "title", value: v }]); }, { placeholder: "Untitled" })));
        inspector.appendChild(field("Description", textInput(d.description, function (v) { applyOps([{ op: "set", key: "description", value: v }]); })));
        inspector.appendChild(field("Layout", selectInput([["layered", "Flow (layered)"], ["grid", "Grid"]], s.layout || "layered", function (v) { applyOps([{ op: "set", key: "layout", value: v }]); })));
        inspector.appendChild(h("p", { class: "dge-muted" }, [d.nodes.length + " nodes · " + (d.edges || []).length + " edges · " + (d.groups || []).length + " groups"]));
        inspector.appendChild(h("p", { class: "dge-tip" }, ["Double-click the canvas to add a box. Drag a box's dot onto another box to connect them, or onto empty space to make a new one. Press ? for shortcuts."]));
        return;
      }
      if (sel.length > 1) {
        inspector.appendChild(h("h3", {}, [sel.length + " selected"]));
        var nodes = sel.filter(node);
        inspector.appendChild(field("Colour", swatches(null, function (c) {
          applyOps(sel.map(function (id) { return node(id) ? { op: "update_node", id: id, color: c } : edge(id) ? { op: "update_edge", id: id, color: c } : { op: "update_group", id: id, color: c }; }));
        })));
        if (nodes.length) inspector.appendChild(field("Shape", selectInput([""].concat(SHAPES), "", function (v) { if (v) applyOps(nodes.map(function (id) { return { op: "update_node", id: id, shape: v }; })); })));
        inspector.appendChild(h("div", { class: "dge-actions" }, [
          h("button", { onclick: groupSelection }, ["Group (" + MOD + "G)"]),
          h("button", { onclick: function () { applyOps(nodes.map(function (id) { return { op: "unpin", id: id }; })); } }, ["Auto-place"]),
          h("button", { class: "danger", onclick: deleteSelection }, ["Delete"])
        ]));
        return;
      }
      var id = sel[0], kind = kindOf(id);
      var commentBtn = h("button", { onclick: function () { st.composer = { on: id }; renderPanel(); var ta = commentsEl.querySelector(".dge-composer textarea"); if (ta) ta.focus(); } }, ["Comment"]);
      if (kind === "node") {
        var n = node(id);
        inspector.appendChild(h("h3", {}, ["Node ", h("code", {}, [id])]));
        inspector.appendChild(field("Label", textInput(n.label || n.id, function (v) { applyOps([{ op: "update_node", id: id, label: v }]); })));
        inspector.appendChild(field("Detail", textInput(n.detail, function (v) { applyOps([{ op: "update_node", id: id, detail: v }]); }, { placeholder: "e.g. Rust · Axum" })));
        inspector.appendChild(field("Shape", selectInput(SHAPES, n.shape || "box", function (v) { applyOps([{ op: "update_node", id: id, shape: v }]); })));
        inspector.appendChild(field("Colour", swatches(n.color, function (c) { applyOps([{ op: "update_node", id: id, color: c }]); })));
        inspector.appendChild(field("Group", selectInput([["", "—"]].concat((st.doc.groups || []).map(function (g) { return [g.id, g.label || g.id]; })), n.group || "", function (v) { applyOps([{ op: "update_node", id: id, group: v }]); })));
        inspector.appendChild(field("Link", textInput(n.link, function (v) { applyOps([{ op: "update_node", id: id, link: v }]); }, { placeholder: "https://…" })));
        inspector.appendChild(field("Tags", textInput((n.tags || []).join(" "), function (v) { applyOps([{ op: "update_node", id: id, tags: v.split(/\s+/).filter(Boolean) }]); }, { placeholder: "service team-a" })));
        inspector.appendChild(field("Id", textInput(id, function (v) { var res = applyOps([{ op: "rename", id: id, to: v.trim() }]); if (res.ok) select([v.trim()]); })));
        var acts = [commentBtn];
        if (n.x != null) acts.push(h("button", { title: "Let the layout place it", onclick: function () { applyOps([{ op: "unpin", id: id }]); } }, ["Auto-place"]));
        acts.push(h("button", { class: "danger", onclick: deleteSelection }, ["Delete"]));
        inspector.appendChild(h("div", { class: "dge-actions" }, acts));
      } else if (kind === "edge") {
        var e = edge(id);
        inspector.appendChild(h("h3", {}, ["Edge ", h("code", {}, [e.from + " → " + e.to])]));
        inspector.appendChild(field("Label", textInput(e.label, function (v) { applyOps([{ op: "update_edge", id: id, label: v }]); })));
        inspector.appendChild(field("Line", selectInput(["solid", "dashed", "dotted", "thick"], e.line || "solid", function (v) { applyOps([{ op: "update_edge", id: id, line: v }]); })));
        inspector.appendChild(field("Arrow", selectInput([["end", "→ end"], ["start", "← start"], ["both", "↔ both"], ["none", "— none"]], e.arrow || "end", function (v) { applyOps([{ op: "update_edge", id: id, arrow: v }]); })));
        inspector.appendChild(field("Colour", swatches(e.color, function (c) { applyOps([{ op: "update_edge", id: id, color: c }]); })));
        inspector.appendChild(h("div", { class: "dge-actions" }, [
          commentBtn,
          h("button", { onclick: function () { var r = applyOps([{ op: "update_edge", id: id, from: e.to, to: e.from }]); if (r.ok) select([]); } }, ["Reverse"]),
          h("button", { class: "danger", onclick: deleteSelection }, ["Delete"])
        ]));
      } else {
        var g = group(id);
        inspector.appendChild(h("h3", {}, ["Group ", h("code", {}, [id])]));
        inspector.appendChild(field("Label", textInput(g.label || g.id, function (v) { applyOps([{ op: "update_group", id: id, label: v }]); })));
        inspector.appendChild(field("Colour", swatches(g.color, function (c) { applyOps([{ op: "update_group", id: id, color: c }]); })));
        inspector.appendChild(field("Inside", selectInput([["", "—"]].concat((st.doc.groups || []).filter(function (x) { return x.id !== id; }).map(function (x) { return [x.id, x.label || x.id]; })), g.parent || "", function (v) { applyOps([{ op: "update_group", id: id, parent: v }]); })));
        inspector.appendChild(h("div", { class: "dge-actions" }, [
          commentBtn,
          h("button", { onclick: function () { applyOps([{ op: "remove", id: id, keep_members: true }]); select([]); } }, ["Ungroup"]),
          h("button", { class: "danger", onclick: function () { applyOps([{ op: "remove", id: id, keep_members: false }]); select([]); } }, ["Delete with contents"])
        ]));
      }
    }

    function renderComments() {
      commentsEl.innerHTML = "";
      if (!st.doc) return;
      var all = st.doc.comments || [];
      var open = all.filter(function (c) { return c.status !== "resolved"; });
      var head = h("div", { class: "dge-comments-head" }, [
        h("h3", {}, ["Comments ", h("span", { class: "dge-count" }, [String(open.length)])]),
        h("label", { class: "dge-check" }, [h("input", { type: "checkbox", checked: st.showResolved, onchange: function (e) { st.showResolved = e.target.checked; render(); } }), " resolved"])
      ]);
      commentsEl.appendChild(head);
      if (!view) {
        commentsEl.appendChild(h("div", { class: "dge-actions" }, [
          h("button", { class: "dge-primary", disabled: !open.length, title: "Turn open comments into an LLM prompt", onclick: function () { openPrompt(); } }, ["Copy as prompt…"]),
          h("button", { title: "Paste an LLM reply (ops JSON or a diagram) to apply it", onclick: openApply }, ["Apply reply…"])
        ]));
      }
      if (!st.author && !view) {
        var nameIn = textInput("", function (v) { st.author = v.trim(); lsSet("diagram.author", st.author); renderComments(); }, { placeholder: "Your name (for comments)" });
        commentsEl.appendChild(nameIn);
      }
      if (st.composer) {
        var ta = h("textarea", { rows: 3, placeholder: st.composer.on ? "Comment on " + labelOf(st.composer.on) + "…" : "Comment on the diagram…" });
        ta.addEventListener("keydown", function (e) {
          e.stopPropagation();
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) post();
          if (e.key === "Escape") { st.composer = null; renderComments(); }
        });
        var post = function () {
          var text = ta.value.trim();
          if (!text) return;
          var op = { op: "add_comment", text: text };
          if (st.composer.on) op.on = st.composer.on;
          else { op.x = st.composer.x; op.y = st.composer.y; }
          if (st.author) op.author = st.author;
          st.composer = null;
          var res = applyOps([op]);
          if (res.ok) { st.comment = res.created[0]; markSelection(); renderComments(); }
        };
        commentsEl.appendChild(h("div", { class: "dge-composer" }, [
          ta,
          h("div", { class: "dge-actions" }, [
            h("button", { class: "dge-primary", onclick: post }, ["Comment (" + MOD + "↵)"]),
            h("button", { onclick: function () { st.composer = null; renderComments(); } }, ["Cancel"])
          ])
        ]));
      }
      var list = st.showResolved ? all : open;
      if (!list.length && !st.composer) {
        commentsEl.appendChild(h("p", { class: "dge-muted" }, [view ? "No comments." : "No open comments. Press M and click something to comment on it."]));
      }
      list.forEach(function (c) {
        var num = all.indexOf(c) + 1;
        var resolved = c.status === "resolved";
        var card = h("div", { class: "dge-card" + (resolved ? " resolved" : "") + (st.comment === c.id ? " active" : ""), "data-cid": c.id });
        card.addEventListener("click", function (e) {
          if (e.target.closest("button,textarea,input")) return;
          st.comment = c.id;
          if (c.on && kindOf(c.on)) st.sel = [c.on];
          markSelection();
          renderPanel();
        });
        var target = c.on ? h("button", { class: "dge-link", onclick: function () { select([c.on]); } }, [labelOf(c.on)]) : h("span", { class: "dge-muted" }, ["canvas"]);
        card.appendChild(h("div", { class: "dge-card-head" }, [h("span", { class: "dge-num" + (resolved ? " done" : "") }, [String(num)]), h("b", {}, [c.author || "anonymous"]), " on ", target, h("code", { class: "dge-cid" }, [c.id])]));
        card.appendChild(h("div", { class: "dge-card-text" }, [c.text]));
        (c.replies || []).forEach(function (r) {
          card.appendChild(h("div", { class: "dge-reply" }, [h("b", {}, [r.author || "anonymous"]), " ", r.text]));
        });
        if (c.resolution) card.appendChild(h("div", { class: "dge-resolution" }, ["✓ " + c.resolution]));
        if (!view) {
          var rin = textInput("", function (v) {
            if (!v.trim()) return;
            var op = { op: "reply", id: c.id, text: v.trim() };
            if (st.author) op.author = st.author;
            applyOps([op]);
          }, { placeholder: "Reply…", class: "dge-reply-in" });
          card.appendChild(rin);
          card.appendChild(h("div", { class: "dge-actions small" }, [
            resolved
              ? h("button", { onclick: function () { applyOps([{ op: "reopen_comment", id: c.id }]); } }, ["Reopen"])
              : h("button", { onclick: function () { applyOps([{ op: "resolve_comment", id: c.id }]); } }, ["Resolve"]),
            h("button", { onclick: function () { openPrompt([c.id]); } }, ["Prompt"]),
            h("button", { class: "danger", onclick: function () { applyOps([{ op: "remove", id: c.id }]); } }, ["Delete"])
          ]));
        }
        commentsEl.appendChild(card);
      });
    }

    function renderPanel() {
      if (panel.hidden) return;
      renderInspector();
      renderComments();
    }

    // ------------------------------------------------- menus, modals, toast
    var toastTimer = null;
    function flash(msg, isError, ms) {
      toast.textContent = msg;
      toast.className = "dge-toast show" + (isError ? " error" : "");
      clearTimeout(toastTimer);
      toastTimer = setTimeout(function () { toast.className = "dge-toast"; }, ms || (isError ? 5000 : 1800));
    }

    function closeMenu() { menu.hidden = true; }
    function openMenu(anchor, items) {
      menu.innerHTML = "";
      items.forEach(function (it) {
        if (it === "-") return menu.appendChild(h("hr"));
        menu.appendChild(h("button", { onclick: function () { closeMenu(); it[1](); } }, [it[0]]));
      });
      var r = anchor.getBoundingClientRect(), host = el.getBoundingClientRect();
      menu.hidden = false;
      menu.style.top = (r.bottom - host.top + 4) + "px";
      menu.style.left = Math.max(8, Math.min(r.left - host.left, host.width - menu.offsetWidth - 8)) + "px";
    }
    document.addEventListener("pointerdown", function (e) { if (!menu.hidden && !menu.contains(e.target) && !e.target.closest(".dge-toolbar")) closeMenu(); });

    function baseName() {
      return slug((st.doc && st.doc.title) || opts.name || "diagram");
    }
    function openExport(anchor) {
      var items = [
        ["Download SVG", function () { download(baseName() + ".svg", exportSVG(), "image/svg+xml"); }],
        ["Download PNG", function () { exportPNG(2).then(function (b) { download(baseName() + ".png", b); }); }],
        ["Copy PNG to clipboard", function () {
          if (!root.ClipboardItem) return flash("This browser cannot copy images", true);
          navigator.clipboard.write([new ClipboardItem({ "image/png": exportPNG(2) })]).then(function () { flash("PNG copied"); }, function (e) { flash(String(e), true); });
        }],
        ["Copy SVG", function () { copyText(exportSVG()).then(function () { flash("SVG copied"); }); }],
        "-",
        ["Download .diagram", function () { download(baseName() + ".diagram", st.text); }],
        ["Download JSON", function () { download(baseName() + ".json", st.session.source("json"), "application/json"); }],
        ["Copy as Mermaid", function () { copyText(st.session.source("mermaid")).then(function () { flash("Mermaid copied — paste it into GitHub Markdown"); }); }],
        ["Copy LLM brief (Markdown)", function () { copyText(st.session.brief()).then(function () { flash("Brief copied — paste it into a chat as context"); }); }]
      ];
      if (opts.shareUrl) {
        items.push("-");
        items.push(["Copy share link", function () {
          encodeShare(st.text).then(function (hash) { return copyText(opts.shareUrl.replace(/#.*$/, "") + "#" + hash); }).then(function () { flash("Link copied — the diagram is in the URL"); });
        }]);
      }
      openMenu(anchor, items);
    }

    function showModal(title, content, actions) {
      modal.innerHTML = "";
      var box = h("div", { class: "dge-dialog", role: "dialog", "aria-label": title }, [
        h("div", { class: "dge-dialog-head" }, [h("h3", {}, [title]), h("button", { class: "dge-x", title: "Close (Esc)", onclick: closeModal }, ["×"])]),
        content,
        h("div", { class: "dge-actions end" }, actions)
      ]);
      modal.appendChild(box);
      modal.hidden = false;
      box.tabIndex = -1;
      box.focus({ preventScroll: true });
      modal.onclick = function (e) { if (e.target === modal) closeModal(); };
      box.addEventListener("keydown", function (e) { e.stopPropagation(); if (e.key === "Escape") closeModal(); });
    }
    function closeModal() { modal.hidden = true; modal.innerHTML = ""; canvas.focus({ preventScroll: true }); }

    function openPrompt(only) {
      var all = (st.doc && st.doc.comments) || [];
      var state = { target: "diagram", reply: "ops", include_resolved: false, comments: only ? only.slice() : [], instructions: "" };
      var preview = h("textarea", { class: "dge-preview", readonly: true, rows: 14 });
      function update() {
        var o = { target: state.target, reply: state.reply, include_resolved: state.include_resolved, comments: state.comments, instructions: state.instructions || null, file: opts.file || null };
        preview.value = st.session.prompt(JSON.stringify(o));
      }
      function radio(name, value, label, desc) {
        var input = h("input", { type: "radio", name: name, value: value, checked: state[name] === value, onchange: function () { state[name] = value; update(); } });
        return h("label", { class: "dge-radio" }, [input, h("span", {}, [h("b", {}, [label]), desc ? h("small", {}, [desc]) : null])]);
      }
      var checks = h("div", { class: "dge-checklist" });
      all.forEach(function (c, i) {
        var inp = h("input", { type: "checkbox", checked: state.comments.indexOf(c.id) >= 0 || (!state.comments.length && c.status !== "resolved"), onchange: function () {
          var chosen = Array.prototype.slice.call(checks.querySelectorAll("input:checked")).map(function (x) { return x.value; });
          state.comments = chosen;
          update();
        } });
        inp.value = c.id;
        checks.appendChild(h("label", { class: "dge-check" + (c.status === "resolved" ? " resolved" : "") }, [inp, " " + (i + 1) + ". " + c.text.slice(0, 80)]));
      });
      var extra = h("textarea", { rows: 2, placeholder: "Extra instructions (optional), e.g. “keep it to one service”" });
      extra.addEventListener("input", function () { state.instructions = extra.value; update(); });
      var content = h("div", { class: "dge-prompt" }, [
        h("div", { class: "dge-cols" }, [
          h("div", {}, [
            h("h4", {}, ["The agent should…"]),
            radio("target", "diagram", "Update the diagram", "reply with edits you apply here"),
            radio("target", "code", "Change the code", "for Claude Code, Codex & co. in your repo"),
            radio("target", "discuss", "Discuss", "answer questions, change nothing"),
            h("h4", {}, ["Reply format"]),
            radio("reply", "ops", "Operations (JSON)", "precise; applied atomically"),
            radio("reply", "dsl", "Whole diagram", "simplest for weaker models")
          ]),
          h("div", {}, [h("h4", {}, ["Comments"]), all.length ? checks : h("p", { class: "dge-muted" }, ["No comments yet."]), extra])
        ]),
        preview
      ]);
      update();
      showModal("Prompt from comments", content, [
        h("button", { onclick: function () { download(baseName() + "-prompt.md", preview.value, "text/markdown"); } }, ["Download .md"]),
        h("button", { onclick: openApply }, ["Apply reply…"]),
        h("button", { class: "dge-primary", onclick: function () { copyText(preview.value).then(function () { flash("Prompt copied — paste it into your LLM, then use Apply reply"); }); } }, ["Copy prompt"])
      ]);
    }

    function openApply() {
      var ta = h("textarea", { class: "dge-preview", rows: 14, placeholder: "Paste the LLM's reply here: a ```json {\"ops\": [...]} block, or a ```diagram block with the whole diagram." });
      var out = h("div", { class: "dge-muted" });
      showModal("Apply an LLM reply", h("div", {}, [ta, out]), [
        h("button", { onclick: closeModal }, ["Cancel"]),
        h("button", { class: "dge-primary", onclick: function () {
          var res = applyReply(ta.value);
          if (res.ok) { closeModal(); flash(res.message); }
          else out.textContent = res.error;
        } }, ["Apply"])
      ]);
      setTimeout(function () { ta.focus(); }, 0);
    }

    /* Apply a chat reply: ops JSON, or a whole diagram in a ```diagram block. */
    function applyReply(text) {
      var m = /```(?:diagram|dsl)\s*\n([\s\S]*?)```/.exec(text);
      if (m && !/```json/.test(text)) {
        var test = new wasm.Diagram();
        var r = JSON.parse(test.set_source(m[1]));
        test.free();
        if (!r.ok) return { ok: false, error: r.errors.map(function (e) { return "line " + e.line + ": " + e.message; }).join("\n") };
        load(m[1], true);
        afterChange(true);
        return { ok: true, message: "Diagram replaced from the reply" };
      }
      var before = st.text;
      var out = JSON.parse(st.session.apply(text));
      if (!out.ok) {
        st.session.set_source(before);
        refreshDoc();
        return { ok: false, error: out.error };
      }
      st.undo.push(before);
      st.redo = [];
      refreshDoc();
      st.text = st.session.source("dsl");
      afterChange(true);
      return { ok: true, message: "Applied " + out.count + " changes" };
    }

    // -------------------------------------------------------------- exports
    function exportSVG() {
      return JSON.parse(st.session.render(JSON.stringify({ comments: "none", theme: st.dark ? "dark" : "light" }))).svg;
    }
    function exportPNG(scale) {
      scale = scale || 2;
      var r = JSON.parse(st.session.render(JSON.stringify({ comments: "none", theme: st.dark ? "dark" : "light" })));
      return new Promise(function (resolve, reject) {
        var img = new Image();
        var url = URL.createObjectURL(new Blob([r.svg], { type: "image/svg+xml" }));
        img.onload = function () {
          var c = document.createElement("canvas");
          c.width = Math.ceil(r.view.w * scale);
          c.height = Math.ceil(r.view.h * scale);
          var ctx = c.getContext("2d");
          ctx.scale(scale, scale);
          ctx.drawImage(img, 0, 0, r.view.w, r.view.h);
          URL.revokeObjectURL(url);
          c.toBlob(function (b) { b ? resolve(b) : reject(new Error("PNG export failed")); }, "image/png");
        };
        img.onerror = function () { URL.revokeObjectURL(url); reject(new Error("PNG export failed")); };
        img.src = url;
      });
    }

    function setBase(src, label) {
      st.base = src;
      var r = JSON.parse(st.session.set_base(src == null ? undefined : src));
      if (!r.ok) flash("Base version: " + (r.errors || []).map(function (e) { return e.message; }).join("; "), true);
      toolbar.baseChip.hidden = src == null;
      toolbar.baseChip.innerHTML = "";
      if (src != null) {
        toolbar.baseChip.appendChild(document.createTextNode("Changes since " + (label || "base") + " "));
        toolbar.baseChip.appendChild(h("button", { title: "Copy the change summary", onclick: function () { copyText(st.session.diff_markdown()).then(function () { flash("Change summary copied"); }); } }, ["copy"]));
        toolbar.baseChip.appendChild(h("button", { title: "Stop comparing", onclick: function () { setBase(null); } }, ["×"]));
      }
      render();
    }

    // ------------------------------------------------------------------ api
    var resizeObs = root.ResizeObserver ? new ResizeObserver(function () { if (!st.fitted && st.r) { fit(); st.fitted = true; } }) : null;
    if (resizeObs) resizeObs.observe(canvas);

    var api = {
      getSource: function (fmt) { return !fmt || fmt === "dsl" ? st.text : st.session.source(fmt); },
      setSource: function (src, o) {
        load(src, !(o && o.silent === true) && !!st.doc);
        syncCode(true);
        showErrors();
        render();
        if (o && o.fit) fit();
      },
      apply: function (ops) { return applyReply(typeof ops === "string" ? ops : JSON.stringify(Array.isArray(ops) ? { ops: ops } : ops)); },
      getDocument: function () { return JSON.parse(st.session.document()); },
      errors: function () { return st.errors.slice(); },
      setBase: setBase,
      prompt: function (o) { return st.session.prompt(JSON.stringify(o || {})); },
      brief: function () { return st.session.brief(); },
      exportSVG: exportSVG,
      exportPNG: exportPNG,
      select: function (ids) { select(ids || []); },
      fit: fit,
      setTheme: function (t) { st.dark = t === "dark"; applyChrome(); render(); },
      setMode: function (m) { view = m === "view"; el.classList.toggle("dge-view", view); buildToolbar(); applyChrome(); render(); },
      addButton: function (b) {
        var button = h("button", { title: b.title || "", onclick: function (e) { b.onClick(e, api); } }, [b.label]);
        (st.extraButtons = st.extraButtons || []).push(button);
        toolbar.extra.appendChild(button);
        return button;
      },
      flash: flash,
      on: function (name, fn) { (listeners[name] = listeners[name] || []).push(fn); return api; },
      destroy: function () {
        document.removeEventListener("keydown", onKey);
        document.removeEventListener("keyup", onKeyUp);
        if (resizeObs) resizeObs.disconnect();
        st.session.free();
        el.innerHTML = "";
        el.classList.remove("dge", "dge-view");
      },
      session: st.session
    };

    buildToolbar();
    buildTabs();
    applyChrome();
    load(opts.source || "", false);
    if (opts.base != null) {
      st.session.set_base(opts.base);
      st.base = opts.base;
    }
    syncCode(true);
    showErrors();
    render();
    if (opts.base != null) setBase(opts.base, opts.baseLabel);
    return api;
  }

  root.DiagramEditor = { create: create, encodeShare: encodeShare, decodeShare: decodeShare, slug: slug };
})(typeof window !== "undefined" ? window : this);
