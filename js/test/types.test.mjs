// The op types in index.d.ts follow the Rust `Op` enum (crates/core/src/ops.rs):
// the same ops, the same fields, the same required ones. Skipped outside the repo.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const rs = fileURLToPath(new URL("../../crates/core/src/ops.rs", import.meta.url));
const dts = fileURLToPath(new URL("../index.d.ts", import.meta.url));
const snake = (s) => s.replace(/[A-Z]/g, (c, i) => (i ? "_" : "") + c.toLowerCase());

/** {op: {field: required}} from `pub enum Op { … }`. */
function rustOps(src) {
  const body = src.slice(src.indexOf("pub enum Op {"));
  const lines = body.slice(0, body.indexOf("\n}\n")).split("\n").slice(1);
  const ops = {};
  let cur = null, dflt = false;
  for (const l of lines) {
    let m;
    // `Name { a: String, b: Option<String> },` on one line
    if ((m = /^    ([A-Z]\w*)\s*\{(.*)\},$/.exec(l))) {
      cur = ops[snake(m[1])] = {};
      for (const f of m[2].split(/,(?![^<]*>)/).map((x) => x.trim()).filter(Boolean)) {
        const [k, ty] = f.split(/:\s*/);
        cur[k] = !(/^(Option|Vec)</.test(ty) || ty === "bool");
      }
      continue;
    }
    if ((m = /^    ([A-Z]\w*)\s*[{,]/.exec(l))) { cur = ops[snake(m[1])] = {}; dflt = false; continue; }
    if (/^\s*#\[serde\(.*default/.test(l)) { dflt = true; continue; }
    if ((m = /^        (\w+): (.+),$/.exec(l)) && cur) {
      const ty = m[2];
      cur[m[1]] = !(dflt || /^(Option|Vec)</.test(ty) || ty === "bool");
      dflt = false;
    }
  }
  return ops;
}

/** {op: {field: required}} from the `export interface …Op` blocks. */
function tsOps(src) {
  const ops = {};
  for (const m of src.matchAll(/export interface \w+Op \{([\s\S]*?)\n\}/g)) {
    const body = m[1].replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
    const name = /\bop: "(\w+)"/.exec(body)[1];
    const fields = {};
    for (const f of body.matchAll(/^\s*(\w+)(\??):/gm)) if (f[1] !== "op") fields[f[1]] = f[2] !== "?";
    ops[name] = fields;
  }
  return ops;
}

test("index.d.ts ops match the Rust Op enum", { skip: !existsSync(rs) && "not in the repository" }, () => {
  const r = rustOps(readFileSync(rs, "utf8")), t = tsOps(readFileSync(dts, "utf8"));
  assert.ok(Object.keys(r).length > 15, "parsed the Rust ops");
  assert.deepEqual(Object.keys(t).sort(), Object.keys(r).sort(), "op names");
  for (const op of Object.keys(r)) assert.deepEqual(t[op], r[op], `fields of ${op} (true = required)`);
  // and the Op union lists every one
  const union = /export type Op =([^;]+);/.exec(readFileSync(dts, "utf8"))[1];
  for (const name of Object.keys(t)) {
    const iface = /export interface (\w+Op) \{[^}]*?\bop: "(\w+)"/g;
    for (const m of readFileSync(dts, "utf8").matchAll(iface)) if (m[2] === name) assert.match(union, new RegExp(`\\b${m[1]}\\b`), `${m[1]} in Op`);
  }
});
