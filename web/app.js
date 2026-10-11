/* The diagram app shell.
 *
 * Served by `diagram FILE`: loads the file from the local server, saves on
 * every change and picks up edits made in other tools (your $EDITOR, an agent).
 * Static (window.DIAGRAM_STATIC, the GitHub Pages playground): examples, local
 * drafts, opening files and share links that carry the diagram in the URL.
 */
(function () {
  "use strict";
  var STATIC = window.DIAGRAM_STATIC || null;
  var SHARE_URL = (STATIC && STATIC.shareUrl) || "https://dobrinov.github.io/diagram/app/";
  var statusEl = document.getElementById("status");
  var fileEl = document.getElementById("file");
  var actions = document.getElementById("app-actions");
  var editor = null;

  function status(text, cls) {
    statusEl.textContent = text;
    statusEl.className = "status" + (cls ? " " + cls : "");
  }

  function boot() {
    return wasm_bindgen({ module_or_path: "pkg/diagram_wasm_bg.wasm" }).then(function () { return wasm_bindgen; });
  }

  function mount(wasm, source, extra) {
    var host = document.getElementById("editor");
    host.innerHTML = "";
    editor = DiagramEditor.create(host, Object.assign({ wasm: wasm, source: source, shareUrl: SHARE_URL }, extra));
    window.diagramEditor = editor; // handy from the console
    return editor;
  }

  // ------------------------------------------------------------ server mode
  function serverMode(wasm) {
    var mtime = null, saving = false, pending = null, timer = null, lastSaved = null;
    function save(text) {
      if (saving) { pending = text; return; }
      saving = true;
      status("Saving…");
      fetch("api/file", { method: "PUT", body: text }).then(function (r) { return r.json(); }).then(function (res) {
        saving = false;
        if (!res.ok) throw new Error(res.error);
        mtime = res.mtime;
        lastSaved = text;
        status("Saved");
        if (pending != null) { var p = pending; pending = null; save(p); }
      }).catch(function (e) { saving = false; status("Not saved: " + e.message, "error"); });
    }
    var ai = fetch("api/ai").then(function (r) { return r.json(); }).catch(function () { return {}; });
    return Promise.all([fetch("api/file").then(function (r) { return r.json(); }), ai]).then(function (both) {
      var f = both[0], harness = both[1];
      mtime = f.mtime;
      lastSaved = f.source;
      fileEl.textContent = f.path;
      fileEl.title = f.path;
      document.title = f.name + " — diagram";
      mount(wasm, f.source, {
        author: f.author || undefined,
        file: f.path,
        name: f.name.replace(/\..*$/, ""),
        // ✦ actions go to the harness the server was started with (--ai CMD)
        ai: harness.available ? function (prompt) {
          return fetch("api/ai", { method: "POST", body: prompt }).then(function (r) { return r.json(); }).then(function (res) {
            if (!res.ok) throw new Error(res.error);
            return res.text;
          });
        } : null,
        onChange: function (text) {
          if (text === lastSaved) return;
          status("Unsaved", "dirty");
          clearTimeout(timer);
          timer = setTimeout(function () { save(text); }, 400);
        },
        onSave: function (text) { clearTimeout(timer); save(text); }
      });
      status("Saved");
      editor.addButton({ label: "Compare ▾", title: "Highlight what changed since a git version", onClick: function () {
        var ref = window.prompt("Show changes since which git version? (HEAD, main, HEAD~3, a commit…)", "HEAD");
        if (!ref) return;
        fetch("api/git?ref=" + encodeURIComponent(ref)).then(function (r) { return r.json(); }).then(function (res) {
          if (!res.ok) return editor.flash(res.error, true);
          editor.setBase(res.source, ref);
        });
      } });
      // pick up changes made outside the editor
      setInterval(function () {
        if (saving || document.hidden) return;
        fetch("api/version").then(function (r) { return r.json(); }).then(function (v) {
          if (v.mtime === mtime || saving) return;
          return fetch("api/file").then(function (r) { return r.json(); }).then(function (f2) {
            mtime = f2.mtime;
            if (f2.source === editor.getSource()) return;
            lastSaved = f2.source;
            editor.setSource(f2.source);
            editor.flash("Reloaded: the file changed on disk");
            status("Saved");
          });
        }).catch(function () { status("Server stopped — changes are not saved", "error"); });
      }, 1500);
    });
  }

  // ------------------------------------------------------------ static mode
  function staticMode(wasm) {
    var examples = STATIC.examples || [];
    var DRAFT = "diagram.draft";
    function draft() { try { return localStorage.getItem(DRAFT); } catch (e) { return null; } }
    function fetchText(url) { return fetch(url).then(function (r) { if (!r.ok) throw new Error(url + ": " + r.status); return r.text(); }); }
    return DiagramEditor.decodeShare(location.hash).catch(function () { return null; }).then(function (shared) {
      var start = shared ? Promise.resolve(shared) : draft() ? Promise.resolve(draft()) : examples.length ? fetchText(examples[0].url) : Promise.resolve("a -> b");
      return start.then(function (src) {
        fileEl.textContent = shared ? "shared diagram" : "playground";
        mount(wasm, src, {
          onChange: function (text) {
            try { localStorage.setItem(DRAFT, text); } catch (e) { /* private mode */ }
            status("Draft saved in this browser");
          }
        });
        if (shared) history.replaceState(null, "", location.pathname);
        var sel = document.createElement("select");
        sel.title = "Examples";
        sel.innerHTML = '<option value="">Examples…</option>' + examples.map(function (e, i) { return '<option value="' + i + '">' + e.name + "</option>"; }).join("");
        sel.onchange = function () {
          var ex = examples[+sel.value];
          sel.value = "";
          if (ex) fetchText(ex.url).then(function (t) { editor.setSource(t, { fit: true }); fileEl.textContent = ex.name; });
        };
        var open = document.createElement("label");
        open.className = "btn";
        open.textContent = "Open…";
        var input = document.createElement("input");
        input.type = "file";
        input.accept = ".diagram,.json,.mmd,.txt,.md";
        input.hidden = true;
        input.onchange = function () {
          var f = input.files[0];
          if (f) f.text().then(function (t) { editor.setSource(t, { fit: true }); fileEl.textContent = f.name; });
        };
        open.appendChild(input);
        var blank = document.createElement("button");
        blank.textContent = "New";
        blank.onclick = function () { editor.setSource("title: Untitled\n", { fit: true }); fileEl.textContent = "playground"; };
        actions.appendChild(sel);
        actions.appendChild(open);
        actions.appendChild(blank);
      });
    });
  }

  boot().then(function (wasm) {
    return STATIC ? staticMode(wasm) : serverMode(wasm);
  }).catch(function (e) {
    document.getElementById("editor").innerHTML = '<p id="boot">Could not start: ' + String(e && e.message || e) + "</p>";
    console.error(e);
  });
})();
