import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "..");
const readme = await readFile(resolve(repositoryRoot, "README.md"), "utf8");
const demo = await readFile(
  resolve(repositoryRoot, "docs/demo/index.html"),
  "utf8",
);
const legal = await readFile(resolve(repositoryRoot, "docs/legal.md"), "utf8");

assert.match(
  readme,
  /<img[^>]+src=["']docs\/media\/drymark-demo\.gif["'][^>]*>/i,
  "README must embed the animated GIF instead of linking to it as text",
);
assert.match(
  readme,
  /not a compliance bypass[\s\S]+\[Legal and responsible use\]\(docs\/legal\.md\)/i,
  "README must state the legal-use boundary and link its detail",
);
assert.match(
  readme,
  /https:\/\/gabrimatic\.github\.io\/drymark\/demo\//,
  "README must link to the hosted video player",
);
assert.doesNotMatch(
  readme,
  /href=["'][^"']*drymark-demo\.mp4\?raw=1["']/i,
  "README must not present GitHub's generic raw response as an inline player",
);
assert.match(
  readme,
  /---\s+Created by \[Soroush Yousefpour\]\(https:\/\/gabrimatic\.info\)\s+\[!\["Buy Me A Coffee"\]\(https:\/\/www\.buymeacoffee\.com\/assets\/img\/custom_images\/orange_img\.png\)\]\(https:\/\/www\.buymeacoffee\.com\/gabrimatic\)\s*$/,
  "README must keep the standard author and support footer",
);

assert.match(demo, /<video\b[^>]*\bcontrols\b[^>]*>/i);
assert.match(demo, /<video\b[^>]*\bmuted\b[^>]*>/i);
assert.match(demo, /<video\b[^>]*\bplaysinline\b[^>]*>/i);
assert.match(demo, /<source[^>]+src=["']\.\.\/media\/drymark-demo\.mp4["'][^>]+type=["']video\/mp4["']/i);
assert.match(demo, /poster=["']\.\.\/media\/drymark-demo-poster\.png["']/i);
assert.doesNotMatch(demo, /purple|violet|magenta/i);

for (const requiredReference of [
  "https://eur-lex.europa.eu/eli/reg/2024/1689/oj",
  "https://eur-lex.europa.eu/eli/dir/2001/29/oj",
  "https://www.gesetze-im-internet.de/urhg/__95a.html",
  "https://www.gesetze-im-internet.de/urhg/__95c.html",
  "https://www.gesetze-im-internet.de/urhg/__108b.html",
  "https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32026R1744",
  "https://eur-lex.europa.eu/eli/dir/2005/29/oj",
]) {
  assert.ok(legal.includes(requiredReference), `legal note must cite ${requiredReference}`);
}
assert.match(legal, /not legal advice/i);
assert.match(legal, /no reviewed EU rule[\s\S]+inherently unlawful/i);
assert.match(legal, /does[\s\S]+not cancel an independently applicable disclosure duty/i);
assert.match(legal, /open-source licence is not an automatic exemption/i);
assert.match(legal, /until 2 December 2026/i);
assert.match(legal, /Court of[\s\S]+Justice of the European Union/i);

console.log("README embeds working media and keeps the standard public footer");
