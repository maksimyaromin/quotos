#!/usr/bin/env node
/* Rebuilds _ds_bundle.js from the component sources, so the card pages
 * (components/(star)/(star).card.html, ui_kits/quotos/index.html) render the same
 * component generation the app consumes. Run from anywhere:
 *
 *   node docs/design/system/_build_bundle.mjs
 *
 * The bundle contract the pages rely on: one IIFE per source module, each
 * wrapped in try/catch that records failures in the namespace's __errors
 * array; modules share a __ds_scope object for cross-file imports; the
 * public components are exposed on window.QuotosDesignSystem_52e720 at the
 * end. React is a page-provided global (the cards load the UMD build), so
 * "react" imports become destructurings of that global. esbuild does the
 * JSX transform — resolved from the repo root's node_modules; no dependency
 * of its own.
 */
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const { transformSync } = createRequire(join(here, "../../../package.json"))("esbuild");

const NAMESPACE = "QuotosDesignSystem_52e720";

// Dependency order: a module may reference any earlier module's exports.
const MODULES = [
  "components/controls/Button.jsx",
  "components/controls/IconButton.jsx",
  "components/controls/TextField.jsx",
  "components/indicators/Badge.jsx",
  "components/indicators/CapacityBar.jsx",
  "components/indicators/StatusDot.jsx",
  "components/shell/MenuBarTile.jsx",
  "components/shell/Panel.jsx",
  "components/subscription/LimitWindow.jsx",
  "components/subscription/SubscriptionRow.jsx",
  "ui_kits/quotos/data.js",
];

// The public API the pages import from the namespace; every other export
// stays in __ds_scope (recorded in the manifest as unexposedExports).
const EXPOSED = [
  "Button", "IconButton", "TextField",
  "Badge", "CapacityBar", "StatusDot",
  "QuotaGlyph", "MenuBarTile", "Panel",
  "LimitWindow", "SubscriptionRow",
];

/** Strips import/export syntax down to plain declarations against the
 *  page-global React and the shared __ds_scope, collecting exported names. */
function rewriteModule(source, path) {
  const exported = [];
  const out = [];
  for (const line of source.split("\n")) {
    let m;
    if (/^import React from "react";$/.test(line)) continue;
    if ((m = line.match(/^import React, \{ ([^}]+) \} from "react";$/))) {
      out.push(`const { ${m[1]} } = React;`);
      continue;
    }
    if ((m = line.match(/^import \{ ([^}]+) \} from "\.[^"]+";$/))) {
      out.push(`const { ${m[1]} } = __ds_scope;`);
      continue;
    }
    if (/^import /.test(line)) throw new Error(`${path}: unsupported import: ${line}`);
    if ((m = line.match(/^export (?:function|const) (\w+)/))) {
      exported.push(m[1]);
      out.push(line.replace(/^export /, ""));
      continue;
    }
    if (/^export /.test(line)) throw new Error(`${path}: unsupported export: ${line}`);
    out.push(line);
  }
  return { rewritten: out.join("\n"), exported };
}

const sections = [];
const sourceHashes = {};
const componentSource = {};
const unexposedExports = [];

for (const path of MODULES) {
  const source = readFileSync(join(here, path), "utf8");
  sourceHashes[path] = createHash("sha256").update(source).digest("hex").slice(0, 12);
  const { rewritten, exported } = rewriteModule(source, path);
  const { code } = transformSync(rewritten, { loader: "jsx", jsx: "transform", target: "es2020" });
  for (const name of exported) {
    if (EXPOSED.includes(name)) componentSource[name] = path;
    else unexposedExports.push({ name, sourcePath: path });
  }
  const register = exported.length ? `Object.assign(__ds_scope, { ${exported.join(", ")} });\n` : "";
  sections.push(
    `// ${path}\n` +
      `try { (() => {\n${code}${register}` +
      `})(); } catch (e) { __ds_ns.__errors.push({ path: ${JSON.stringify(path)}, error: String((e && e.message) || e) }); }`,
  );
}

const missing = EXPOSED.filter((name) => !componentSource[name]);
if (missing.length) throw new Error(`exposed components never exported: ${missing.join(", ")}`);

const manifest = {
  format: 4,
  namespace: NAMESPACE,
  components: EXPOSED.map((name) => ({ name, sourcePath: componentSource[name] })),
  sourceHashes,
  inlinedExternals: [],
  unexposedExports,
};

const bundle =
  `/* @ds-bundle: ${JSON.stringify(manifest)} */\n\n` +
  `(() => {\n\n` +
  `const __ds_ns = (window.${NAMESPACE} = window.${NAMESPACE} || {});\n\n` +
  `const __ds_scope = {};\n\n` +
  `(__ds_ns.__errors = __ds_ns.__errors || []);\n\n` +
  sections.join("\n\n") +
  `\n\n` +
  EXPOSED.map((name) => `__ds_ns.${name} = __ds_scope.${name};`).join("\n\n") +
  `\n\n})();\n`;

const outPath = join(here, "_ds_bundle.js");
writeFileSync(outPath, bundle);
console.log(`${outPath}: ${bundle.length} bytes, ${MODULES.length} modules, ${EXPOSED.length} exposed`);
