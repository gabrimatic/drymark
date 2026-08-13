import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";

const require = createRequire(import.meta.url);
const { __test = {}, inspect, sourceText, visibleText } = require("./demo.js");

const DISPLAYED_PARAGRAPH =
  "This synthetic paragraph stays visibly unchanged while DryMark removes supported hidden clipboard channels.";

test("source fixture exposes seven generically detected hidden scalars", () => {
  assert.deepEqual(inspect(sourceText), {
    count: 7,
    defaultIgnorableCount: 6,
    noncharacterCount: 1,
  });
});

test("fixture carriers do not change the displayed paragraph", () => {
  assert.equal(visibleText(sourceText), DISPLAYED_PARAGRAPH);
});

test("cleaned fixture reports no hidden scalars", () => {
  assert.deepEqual(inspect(DISPLAYED_PARAGRAPH), {
    count: 0,
    defaultIgnorableCount: 0,
    noncharacterCount: 0,
  });
});

test("copy success requires a complete synchronous browser receipt", () => {
  assert.equal(typeof __test.copyStatus, "function");
  const complete = {
    commandSucceeded: true,
    eventReceived: true,
    htmlSet: true,
    plainSet: true,
  };

  assert.equal(__test.copyStatus(complete), "Browser accepted marked fixture copy");
  for (const missing of Object.keys(complete)) {
    assert.equal(
      __test.copyStatus({ ...complete, [missing]: false }),
      "Copy not confirmed by browser",
    );
  }
});

test("paste claims supported cleanup only for exact fixture values", () => {
  assert.equal(typeof __test.classifyPaste, "function");
  assert.deepEqual(__test.classifyPaste(DISPLAYED_PARAGRAPH, ["text/plain"]), {
    countLabel: "0 supported hidden channels detected",
    state: "clean",
    statusLabel: "Verified cleaned fixture pasted",
    visibleLabel: "Visible text unchanged for this fixture",
  });
  assert.deepEqual(__test.classifyPaste(sourceText, ["text/plain", "text/html"]), {
    countLabel: "7 supported hidden channels detected",
    state: "marked",
    statusLabel: "Marked fixture pasted — cleanup not verified",
    visibleLabel: "Visible text unchanged for this fixture",
  });

  const generic = __test.classifyPaste("Other\u2065 synthetic text", ["text/plain"]);
  assert.equal(generic.countLabel, "1 generic Unicode-property scalar detected");
  assert.equal(generic.state, "unverified");
  assert.equal(generic.statusLabel, "Paste inspected — fixture cleanup not verified");
  assert.doesNotMatch(generic.countLabel, /supported/i);
  assert.doesNotMatch(generic.statusLabel, /cleaned/i);
});

test("paste requires plain text and never calls arbitrary zero-count text cleaned", () => {
  assert.equal(typeof __test.classifyPaste, "function");
  assert.deepEqual(__test.classifyPaste("Other synthetic text", ["text/plain"]), {
    countLabel: "0 generic Unicode-property scalars detected",
    state: "unverified",
    statusLabel: "Paste inspected — fixture cleanup not verified",
    visibleLabel: "Visible text differs from this fixture",
  });
  assert.deepEqual(__test.classifyPaste("ignored", ["text/html"]), {
    countLabel: "Plain-text clipboard data required",
    state: "unsupported",
    statusLabel: "Paste not inspected",
    visibleLabel: "Visible-text comparison unavailable",
  });
});
