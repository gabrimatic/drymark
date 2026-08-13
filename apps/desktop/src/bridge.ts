import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import type { DesktopBridge, Policy, DryMarkState } from "./App";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const previewState: DryMarkState = {
  status: "ready",
  policy: "preserve",
  shortcut: "Alt+Shift+V",
  shortcutDisplay: navigator.platform.includes("Mac") ? "⌥ ⇧ V" : "Alt Shift V",
  shortcutStatus: "registered",
  preferencesStatus: "saved",
  visualFeedback: true,
  version: "0.1.0",
  lastResult: {
    kind: "cleaned",
    removed: 7,
    observed: 2,
    canonicalized: false,
    formattingCleared: true,
    at: "Just now",
  },
};

export function createDesktopBridge(): DesktopBridge {
  if (window.__TAURI_INTERNALS__ === undefined) return createPreviewBridge();
  return {
    getState: () => invoke<DryMarkState>("get_state"),
    cleanClipboard: () => invoke<undefined>("clean_clipboard"),
    setPolicy: (policy: Policy) => invoke<undefined>("set_policy", { policy }),
    setShortcut: (shortcut: string) => invoke<undefined>("set_shortcut", { shortcut }),
    setVisualFeedback: (enabled: boolean) => invoke<undefined>("set_visual_feedback", { enabled }),
    openSettings: () => invoke<undefined>("open_settings"),
    quit: () => invoke<undefined>("quit_app"),
    closeCurrentWindow: () => getCurrentWindow().hide().then(() => undefined),
    subscribe: (listener) => listen<DryMarkState>("state-changed", (event) => {
      listener(event.payload);
    }).then((unlisten) => () => {
      unlisten();
      return undefined;
    }),
  };
}

function createPreviewBridge(): DesktopBridge {
  let state = structuredClone(previewState);
  const listeners = new Set<(value: DryMarkState) => void>();
  const publish = () => {
    listeners.forEach((listener) => {
      listener(structuredClone(state));
    });
  };
  return {
    getState: () => Promise.resolve(structuredClone(state)),
    cleanClipboard: () => {
      publish();
      return Promise.resolve(undefined);
    },
    setPolicy: (policy) => {
      state = { ...state, policy };
      publish();
      return Promise.resolve(undefined);
    },
    setShortcut: (shortcut) => {
      state = { ...state, shortcut, shortcutDisplay: shortcut.replaceAll("+", " ") };
      publish();
      return Promise.resolve(undefined);
    },
    setVisualFeedback: (visualFeedback) => {
      state = { ...state, visualFeedback };
      publish();
      return Promise.resolve(undefined);
    },
    openSettings: () => Promise.resolve(undefined),
    quit: () => Promise.resolve(undefined),
    closeCurrentWindow: () => Promise.resolve(undefined),
    subscribe: (listener) => {
      listeners.add(listener);
      return Promise.resolve(() => {
        listeners.delete(listener);
        return undefined;
      });
    },
  };
}
