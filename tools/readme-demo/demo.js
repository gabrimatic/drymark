"use strict";

(() => {
  const fixtureCarriers = new Set([
    "\u200b",
    "\u2060",
    "\ufeff",
    "\u00ad",
    "\u034f",
    "\u2063",
    "\ufdd0",
  ]);
  const defaultIgnorablePattern = /\p{Default_Ignorable_Code_Point}/u;
  const noncharacterPattern = /\p{Noncharacter_Code_Point}/u;

  const sourceText = [
    "This",
    "\u200b",
    " synthetic",
    "\u2060",
    " paragraph",
    "\ufeff",
    " stays",
    "\u00ad",
    " visibly",
    "\u034f",
    " unchanged",
    "\u2063",
    " while DryMark removes supported hidden clipboard channels",
    "\ufdd0",
    ".",
  ].join("");

  function requireString(value) {
    if (typeof value !== "string") {
      throw new TypeError("The fixture inspector accepts text only.");
    }
  }

  function visibleText(value) {
    requireString(value);
    return Array.from(value, (scalar) =>
      fixtureCarriers.has(scalar) ? "" : scalar,
    ).join("");
  }

  function inspect(value) {
    requireString(value);

    let count = 0;
    let defaultIgnorableCount = 0;
    let noncharacterCount = 0;

    for (const scalar of value) {
      const isDefaultIgnorable = defaultIgnorablePattern.test(scalar);
      const isNoncharacter = noncharacterPattern.test(scalar);

      if (isDefaultIgnorable) {
        defaultIgnorableCount += 1;
      }
      if (isNoncharacter) {
        noncharacterCount += 1;
      }
      if (isDefaultIgnorable || isNoncharacter) {
        count += 1;
      }
    }

    return { count, defaultIgnorableCount, noncharacterCount };
  }

  function copyStatus(receipt) {
    return receipt.commandSucceeded &&
      receipt.eventReceived &&
      receipt.htmlSet &&
      receipt.plainSet
      ? "Browser accepted marked fixture copy"
      : "Copy not confirmed by browser";
  }

  function classifyPaste(value, mimeTypes) {
    if (!Array.from(mimeTypes).includes("text/plain")) {
      return {
        countLabel: "Plain-text clipboard data required",
        state: "unsupported",
        statusLabel: "Paste not inspected",
        visibleLabel: "Visible-text comparison unavailable",
      };
    }

    const result = inspect(value);
    const expectedCleanedText = visibleText(sourceText);
    const visibleLabel =
      visibleText(value) === expectedCleanedText
        ? "Visible text unchanged for this fixture"
        : "Visible text differs from this fixture";

    if (value === expectedCleanedText && result.count === 0) {
      return {
        countLabel: "0 supported hidden channels detected",
        state: "clean",
        statusLabel: "Verified cleaned fixture pasted",
        visibleLabel,
      };
    }

    if (value === sourceText && result.count === 7) {
      return {
        countLabel: "7 supported hidden channels detected",
        state: "marked",
        statusLabel: "Marked fixture pasted — cleanup not verified",
        visibleLabel,
      };
    }

    return {
      countLabel: `${result.count} generic Unicode-property ${result.count === 1 ? "scalar" : "scalars"} detected`,
      state: "unverified",
      statusLabel: "Paste inspected — fixture cleanup not verified",
      visibleLabel,
    };
  }

  const api = Object.freeze({ sourceText, visibleText, inspect });

  if (typeof module !== "undefined" && module.exports) {
    module.exports = Object.freeze({
      ...api,
      __test: Object.freeze({ classifyPaste, copyStatus }),
    });
  }

  if (typeof window === "undefined") {
    return;
  }

  window.drymarkDemo = api;

  const mount = () => {
    const sourceParagraph = document.querySelector("[data-source-text]");
    const sourceCount = document.querySelector("[data-source-count]");
    const destinationParagraph = document.querySelector(
      "[data-destination-text]",
    );
    const destinationCount = document.querySelector("[data-destination-count]");
    const visibleResult = document.querySelector("[data-visible-result]");
    const copyStatusElement = document.querySelector("[data-copy-status]");
    const pasteStatus = document.querySelector("[data-paste-status]");
    const copyButton = document.querySelector("[data-copy-button]");
    const pasteButton = document.querySelector("[data-paste-button]");

    sourceParagraph.textContent = visibleText(sourceText);
    sourceCount.textContent = classifyPaste(sourceText, ["text/plain"]).countLabel;

    let activeCopyReceipt = null;
    document.addEventListener("copy", (event) => {
      if (!activeCopyReceipt) {
        return;
      }

      activeCopyReceipt.eventReceived = true;
      if (!event.clipboardData) {
        return;
      }

      try {
        event.preventDefault();
        event.clipboardData.setData("text/plain", sourceText);
        activeCopyReceipt.plainSet = true;
        event.clipboardData.setData("text/html", `<p>${sourceText}</p>`);
        activeCopyReceipt.htmlSet = true;
      } catch {
        activeCopyReceipt.plainSet = false;
        activeCopyReceipt.htmlSet = false;
      }
    });

    copyButton.addEventListener("click", () => {
      const receipt = {
        commandSucceeded: false,
        eventReceived: false,
        htmlSet: false,
        plainSet: false,
      };
      activeCopyReceipt = receipt;
      try {
        receipt.commandSucceeded = document.execCommand("copy") === true;
      } catch {
        receipt.commandSucceeded = false;
      } finally {
        activeCopyReceipt = null;
      }
      copyStatusElement.textContent = copyStatus(receipt);
    });

    destinationParagraph.addEventListener("paste", (event) => {
      const mimeTypes = event.clipboardData
        ? Array.from(event.clipboardData.types)
        : [];
      const hasPlainText = mimeTypes.includes("text/plain");
      const pastedText = hasPlainText
        ? event.clipboardData.getData("text/plain")
        : "";
      const classification = classifyPaste(pastedText, mimeTypes);

      event.preventDefault();
      if (hasPlainText) {
        destinationParagraph.textContent = pastedText;
        destinationParagraph.dataset.empty = "false";
      }

      destinationCount.textContent = classification.countLabel;
      destinationCount.dataset.state = classification.state;
      visibleResult.textContent = classification.visibleLabel;
      visibleResult.dataset.state = classification.state;
      pasteStatus.textContent = classification.statusLabel;
    });

    pasteButton.addEventListener("click", () => {
      destinationParagraph.focus();
      pasteStatus.textContent = "Destination focused — press ⌘V";
    });
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  } else {
    mount();
  }
})();
