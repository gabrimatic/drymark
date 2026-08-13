import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App, type DesktopBridge, type DryMarkState } from "./App";
import styles from "./styles.css?raw";

const readyState: DryMarkState = {
  status: "ready",
  policy: "preserve",
  shortcut: "Alt+Shift+V",
  shortcutDisplay: "⌥ ⇧ V",
  shortcutStatus: "registered",
  preferencesStatus: "saved",
  visualFeedback: true,
  version: "0.1.0",
  lastResult: {
    kind: "cleaned",
    removed: 7,
    observed: 0,
    canonicalized: false,
    formattingCleared: true,
    at: "15:42",
  },
};

function bridge(overrides: Partial<DesktopBridge> = {}): DesktopBridge {
  return {
    getState: vi.fn().mockResolvedValue(readyState),
    cleanClipboard: vi.fn().mockResolvedValue(undefined),
    setPolicy: vi.fn().mockResolvedValue(undefined),
    setShortcut: vi.fn().mockResolvedValue(undefined),
    setVisualFeedback: vi.fn().mockResolvedValue(undefined),
    openSettings: vi.fn().mockResolvedValue(undefined),
    quit: vi.fn().mockResolvedValue(undefined),
    closeCurrentWindow: vi.fn().mockResolvedValue(undefined),
    subscribe: vi.fn().mockResolvedValue(() => undefined),
    ...overrides,
  };
}

test("tray exposes the complete primary workflow accessibly", async () => {
  const desktop = bridge();
  render(<App view="tray" bridge={desktop} />);

  expect(await screen.findByRole("heading", { name: "DryMark" })).toBeVisible();
  expect(screen.getByText("Remove hidden watermarks from LLM text.")).toBeVisible();
  expect(screen.getByText("Ready")).toBeVisible();
  expect(screen.getByLabelText("Current shortcut: Option Shift V")).toBeVisible();
  expect(screen.getByText("7 hidden characters removed · formatting cleared")).toBeVisible();

  fireEvent.click(screen.getByRole("button", { name: "Remove watermarks" }));
  await waitFor(() => {
    expect(desktop.cleanClipboard).toHaveBeenCalledOnce();
  });

  fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
  await waitFor(() => {
    expect(desktop.openSettings).toHaveBeenCalledOnce();
  });
});

test("tray policy control changes policy without ambiguous labels", async () => {
  const desktop = bridge();
  render(<App view="tray" bridge={desktop} />);
  const policies = await screen.findByRole("group", { name: "Watermark removal policy" });

  fireEvent.click(within(policies).getByRole("radio", { name: "Thorough" }));
  await waitFor(() => {
    expect(desktop.setPolicy).toHaveBeenCalledWith("thorough");
  });
});

test("tray shows the presentation warning whenever Thorough is active", async () => {
  const thoroughState = { ...readyState, policy: "thorough" as const };
  const desktop = bridge({ getState: vi.fn().mockResolvedValue(thoroughState) });
  render(<App view="tray" bridge={desktop} />);

  expect(await screen.findByText("Thorough can change emoji, RTL, and specialist notation.")).toBeVisible();
});

test.each(["tray", "settings"] as const)(
  "%s surfaces a preference persistence failure without sensitive detail",
  async (view) => {
    const failedState: DryMarkState = {
      ...readyState,
      preferencesStatus: "write_failed",
    };
    const desktop = bridge({ getState: vi.fn().mockResolvedValue(failedState) });
    render(<App view={view} bridge={desktop} />);

    expect(await screen.findByText("Settings couldn’t be saved. Current session only.")).toBeVisible();
    expect(document.body).not.toHaveTextContent("preferences.json");
  },
);

test("settings explains the guarantee boundary and records a shortcut", async () => {
  const desktop = bridge();
  render(<App view="settings" bridge={desktop} />);

  expect(await screen.findByRole("heading", { name: "General" })).toBeVisible();
  expect(screen.getByText(/never uploads or stores clipboard text/i)).toBeVisible();

  fireEvent.click(screen.getByRole("button", { name: "Remove watermarks now" }));
  await waitFor(() => {
    expect(desktop.cleanClipboard).toHaveBeenCalledOnce();
  });

  const recorder = screen.getByRole("button", { name: "Record a new shortcut" });
  fireEvent.click(recorder);
  fireEvent.keyDown(recorder, { key: "K", code: "KeyK", altKey: true, shiftKey: true });
  await waitFor(() => {
    expect(desktop.setShortcut).toHaveBeenCalledWith("Alt+Shift+K");
  });

  fireEvent.click(recorder);
  fireEvent.keyDown(recorder, { key: "K", code: "KeyK", metaKey: true });
  await waitFor(() => {
    expect(desktop.setShortcut).toHaveBeenCalledWith("Super+K");
  });

  fireEvent.click(screen.getByRole("tab", { name: "Watermarks" }));
  expect(screen.getByRole("heading", { name: "Watermark removal" })).toBeVisible();
  expect(screen.getByText("Choose how aggressively hidden LLM watermark channels are removed.")).toBeVisible();
  expect(screen.getByText(/no lossless policy can remove every invisible channel/i)).toBeVisible();
  expect(screen.getByText(/direction marks, annotation delimiters/i)).toBeVisible();

  fireEvent.click(screen.getByRole("tab", { name: "Privacy" }));
  expect(screen.getByText(/operating-system clipboard services may still sync/i)).toBeVisible();
  expect(screen.queryByText(/clipboard stays on this device, always/i)).not.toBeInTheDocument();
});

test("settings sections expose keyboard-operable tab semantics", async () => {
  render(<App view="settings" bridge={bridge()} />);

  const general = await screen.findByRole("tab", { name: "General" });
  expect(general).toHaveAttribute("aria-selected", "true");
  fireEvent.keyDown(general, { key: "ArrowRight" });

  const cleaning = screen.getByRole("tab", { name: "Watermarks" });
  expect(cleaning).toHaveAttribute("aria-selected", "true");
  await waitFor(() => {
    expect(cleaning).toHaveFocus();
  });
  const panel = screen.getByRole("tabpanel");
  expect(panel).toHaveAttribute("aria-labelledby", "settings-tab-cleaning");
  expect(within(panel).getByRole("heading", { name: "Watermark removal" })).toBeVisible();
});

test("the approved stylesheet contains no purple-family hues", () => {
  expect(styles).not.toMatch(/\b(?:purple|violet|magenta)\b|hue-rotate/i);

  const colors = [...styles.matchAll(/#[0-9a-f]{6}\b/gi)].map(([value]) => value);
  for (const color of colors) {
    const channels = color.slice(1).match(/../g)?.map((value) => Number.parseInt(value, 16));
    if (channels === undefined) continue;
    const [red = 0, green = 0, blue = 0] = channels;
    const maximum = Math.max(red, green, blue);
    const minimum = Math.min(red, green, blue);
    const delta = maximum - minimum;
    if (delta < 24) continue;

    let hue: number;
    if (maximum === red) hue = 60 * (((green - blue) / delta) % 6);
    else if (maximum === green) hue = 60 * ((blue - red) / delta + 2);
    else hue = 60 * ((red - green) / delta + 4);
    if (hue < 0) hue += 360;
    expect(hue < 250 || hue > 345, `${color} falls in the forbidden purple-family hue range`).toBe(true);
  }
});

test("toast is silent, concise, and never renders clipboard content", async () => {
  const desktop = bridge();
  const { container } = render(<App view="toast" bridge={desktop} />);

  expect(await screen.findByText("Watermark channels removed")).toBeVisible();
  expect(screen.getByText("7 hidden characters removed · formatting cleared")).toBeVisible();
  const status = screen.getByRole("status");
  expect(status).toHaveAttribute("aria-live", "polite");
  expect(status).toHaveAttribute("aria-atomic", "true");
  expect(status).toHaveTextContent("Watermark channels removed");
  expect(container).not.toHaveTextContent("PRIVATE-ZXQ");
  expect(container.querySelector("audio")).toBeNull();
});

test("error state names the clipboard race without claiming a clean", async () => {
  const changedState: DryMarkState = {
    ...readyState,
    lastResult: { kind: "clipboard_changed", removed: 0, observed: 0, canonicalized: false, formattingCleared: false, at: "15:43" },
  };
  const desktop = bridge({ getState: vi.fn().mockResolvedValue(changedState) });
  render(<App view="toast" bridge={desktop} />);

  expect(await screen.findByText("Clipboard changed")).toBeVisible();
  expect(screen.getByText("Nothing was overwritten. Try the shortcut again.")).toBeVisible();
  expect(screen.getByRole("status")).toHaveTextContent(
    "Clipboard changedNothing was overwritten. Try the shortcut again.",
  );
  expect(screen.queryByText("Watermark channels removed")).not.toBeInTheDocument();
});

test("toast announces an unverified write without claiming success", async () => {
  const unknownState: DryMarkState = {
    ...readyState,
    lastResult: { kind: "write_unverified", removed: 0, observed: 0, canonicalized: false, formattingCleared: false, at: "15:43" },
  };
  const desktop = bridge({ getState: vi.fn().mockResolvedValue(unknownState) });
  render(<App view="toast" bridge={desktop} />);

  const status = await screen.findByRole("status");
  expect(status).toHaveTextContent("Couldn’t verify clipboard");
  expect(status).toHaveTextContent("Clipboard state may have changed. Check it before pasting.");
  expect(status).not.toHaveTextContent("Watermark channels removed");
});

test("tray never labels a failed transaction as verified plain text", async () => {
  const changedState: DryMarkState = {
    ...readyState,
    lastResult: { kind: "clipboard_changed", removed: 0, observed: 0, canonicalized: false, formattingCleared: false, at: "15:43" },
  };
  const desktop = bridge({ getState: vi.fn().mockResolvedValue(changedState) });
  render(<App view="tray" bridge={desktop} />);

  expect(await screen.findByText("Clipboard changed safely")).toBeVisible();
  expect(screen.getByText("Clipboard changed before the write")).toBeVisible();
  expect(screen.getByText("No clipboard write")).toBeVisible();
  expect(screen.queryByText("Plain text verified")).not.toBeInTheDocument();
});

test("already-clean copy distinguishes removable channels from observed context", async () => {
  const cleanState: DryMarkState = {
    ...readyState,
    lastResult: { kind: "already_clean", removed: 0, observed: 2, canonicalized: false, formattingCleared: false, at: "15:43" },
  };
  const desktop = bridge({ getState: vi.fn().mockResolvedValue(cleanState) });
  render(<App view="toast" bridge={desktop} />);

  expect(await screen.findByText("Already clean")).toBeVisible();
  expect(screen.getByText("No removable watermark channels were found. 2 contextual characters were preserved.")).toBeVisible();
});

test("canonicalization-only success never reports zero hidden characters", async () => {
  const canonicalState: DryMarkState = {
    ...readyState,
    lastResult: {
      kind: "cleaned",
      removed: 0,
      observed: 0,
      canonicalized: true,
      formattingCleared: true,
      at: "15:44",
    },
  };
  const desktop = bridge({ getState: vi.fn().mockResolvedValue(canonicalState) });
  render(<App view="toast" bridge={desktop} />);

  expect(await screen.findByText("text canonicalized · formatting cleared")).toBeVisible();
  expect(screen.queryByText(/0 hidden/i)).not.toBeInTheDocument();
});

test.each(["write_failed", "write_unverified"] as const)(
  "%s warns that clipboard state is unknown",
  async (kind) => {
    const failedState: DryMarkState = {
      ...readyState,
      lastResult: { kind, removed: 0, observed: 0, canonicalized: false, formattingCleared: false, at: "15:45" },
    };
    const desktop = bridge({ getState: vi.fn().mockResolvedValue(failedState) });
    render(<App view="tray" bridge={desktop} />);

    expect(await screen.findAllByText("Clipboard state unknown")).toHaveLength(2);
    expect(screen.queryByText(/nothing was overwritten/i)).not.toBeInTheDocument();
    expect(screen.queryByText("Plain text verified")).not.toBeInTheDocument();
  },
);
