import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  Check,
  ChevronRight,
  CircleAlert,
  FileKey2,
  Info,
  Keyboard,
  LockKeyhole,
  Power,
  ScanText,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  X,
} from "lucide-react";

import "./styles.css";

export type AppView = "tray" | "settings" | "toast";
export type Policy = "preserve" | "thorough";

export type ResultKind =
  | "cleaned"
  | "already_clean"
  | "clipboard_changed"
  | "empty"
  | "non_text"
  | "too_large"
  | "read_failed"
  | "write_failed"
  | "write_unverified"
  | "recheck_failed";

export interface LastResult {
  kind: ResultKind;
  removed: number;
  observed: number;
  canonicalized: boolean;
  formattingCleared: boolean;
  at: string;
}

export interface DryMarkState {
  status: "ready" | "cleaning" | "error";
  policy: Policy;
  shortcut: string;
  shortcutDisplay: string;
  shortcutStatus: "registered" | "conflict" | "unsupported" | "permission_denied" | "invalid";
  preferencesStatus: "saved" | "write_failed";
  visualFeedback: boolean;
  version: string;
  lastResult: LastResult | null;
}

export interface DesktopBridge {
  getState: () => Promise<DryMarkState>;
  cleanClipboard: () => Promise<undefined>;
  setPolicy: (policy: Policy) => Promise<undefined>;
  setShortcut: (shortcut: string) => Promise<undefined>;
  setVisualFeedback: (enabled: boolean) => Promise<undefined>;
  openSettings: () => Promise<undefined>;
  quit: () => Promise<undefined>;
  closeCurrentWindow: () => Promise<undefined>;
  subscribe: (listener: (state: DryMarkState) => void) => Promise<() => undefined>;
}

interface AppProps {
  view: AppView;
  bridge: DesktopBridge;
}

export function App({ view, bridge }: AppProps) {
  const [state, setState] = useState<DryMarkState | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: (() => undefined) | undefined;

    void bridge.getState().then((next) => {
      if (active) setState(next);
    });
    void bridge.subscribe((next) => {
      if (active) setState(next);
    }).then((stop) => {
      unlisten = stop;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [bridge]);

  if (state === null) {
    return <LoadingView view={view} />;
  }

  if (view === "tray") return <TrayView state={state} bridge={bridge} />;
  if (view === "toast") return <ToastView state={state} bridge={bridge} />;
  return <SettingsView state={state} bridge={bridge} />;
}

function LoadingView({ view }: { view: AppView }) {
  return (
    <main className={`app-shell ${view}`} aria-busy="true" aria-label="Loading DryMark">
      <div className="loading-mark"><BrandMark /></div>
    </main>
  );
}

function BrandMark({ small = false }: { small?: boolean }) {
  return (
    <span className={small ? "brand-mark small" : "brand-mark"} aria-hidden="true">
      <span />
      <span />
      <span />
    </span>
  );
}

function TrayView({ state, bridge }: { state: DryMarkState; bridge: DesktopBridge }) {
  const isCleaning = state.status === "cleaning";

  return (
    <main className="app-shell tray">
      <header className="tray-header">
        <div className="brand-lockup">
          <BrandMark />
          <div>
            <h1>DryMark</h1>
            <p>Remove hidden watermarks from LLM text.</p>
          </div>
        </div>
        <button className="icon-button" type="button" aria-label="Close menu" onClick={() => void bridge.closeCurrentWindow()}>
          <X size={17} />
        </button>
      </header>

      <section className="ready-row" aria-label="DryMark status">
        <span className={`status-dot ${state.shortcutStatus}`} />
        <span>{isCleaning ? "Removing" : shortcutStatusLabel(state.shortcutStatus)}</span>
        <ShortcutKeys display={state.shortcutDisplay} />
      </section>

      {state.preferencesStatus === "write_failed" ? <PersistenceWarning /> : null}

      <button
        className="primary-action"
        type="button"
        disabled={isCleaning}
        onClick={() => void bridge.cleanClipboard()}
      >
        <ShieldCheck size={20} />
        <span>{isCleaning ? "Removing watermarks…" : "Remove watermarks"}</span>
        <ChevronRight size={18} />
      </button>

      <section className="last-clean-card" aria-labelledby="last-clean-title">
        <div className="section-eyebrow">
          <span id="last-clean-title">Last result</span>
          <span>{state.lastResult?.at ?? "Not yet"}</span>
        </div>
        <div className="result-main">
          <span className={`result-icon ${lastResultTone(state.lastResult)}`}>
            {lastResultIcon(state.lastResult)}
          </span>
          <div>
            <strong>{lastResultTitle(state.lastResult)}</strong>
            <p>{lastResultDetail(state.lastResult)}</p>
          </div>
        </div>
        <div className="result-details">
          <span><ScanText size={14} /> {lastResultClipboardState(state.lastResult)}</span>
          <span>Local only</span>
        </div>
      </section>

      <PolicyPicker policy={state.policy} onChange={(policy) => void bridge.setPolicy(policy)} compact />
      {state.policy === "thorough" ? (
        <p className="tray-policy-warning"><CircleAlert size={13} /> Thorough can change emoji, RTL, and specialist notation.</p>
      ) : null}

      <footer className="tray-footer">
        <button type="button" aria-label="Open settings" onClick={() => void bridge.openSettings()}>
          <Settings size={17} /> Settings
        </button>
        <button type="button" onClick={() => void bridge.quit()}>
          <Power size={16} /> Quit
        </button>
      </footer>
    </main>
  );
}

function ShortcutKeys({ display }: { display: string }) {
  const tokens = display.split(/\s+/).filter(Boolean);
  const readable = tokens
    .map((token) => ({ "⌥": "Option", "⇧": "Shift", "⌘": "Command", "⌃": "Control" })[token] ?? token)
    .join(" ");
  return (
    <kbd className="shortcut-keys" aria-label={`Current shortcut: ${readable}`}>
      {tokens.map((token) => <span key={token}>{token}</span>)}
    </kbd>
  );
}

function PolicyPicker({
  policy,
  onChange,
  compact = false,
}: {
  policy: Policy;
  onChange: (policy: Policy) => void;
  compact?: boolean;
}) {
  return (
    <fieldset className={compact ? "policy-picker compact" : "policy-picker"}>
      <legend className="sr-only">Watermark removal policy</legend>
      {(["preserve", "thorough"] as const).map((value) => (
        <label key={value}>
          <input
            type="radio"
            name={compact ? "tray-policy" : "settings-policy"}
            value={value}
            checked={policy === value}
            onChange={() => {
              onChange(value);
            }}
          />
          <span>{value === "preserve" ? "Preserve" : "Thorough"}</span>
        </label>
      ))}
    </fieldset>
  );
}

type SettingsSection = "general" | "cleaning" | "privacy" | "about";

function SettingsView({ state, bridge }: { state: DryMarkState; bridge: DesktopBridge }) {
  const [section, setSection] = useState<SettingsSection>("general");
  const sections: Array<{ id: SettingsSection; label: string; icon: typeof Settings }> = [
    { id: "general", label: "General", icon: SlidersHorizontal },
    { id: "cleaning", label: "Watermarks", icon: Sparkles },
    { id: "privacy", label: "Privacy", icon: LockKeyhole },
    { id: "about", label: "About", icon: Info },
  ];

  const moveSection = (event: KeyboardEvent<HTMLButtonElement>, current: SettingsSection) => {
    const currentIndex = sections.findIndex(({ id }) => id === current);
    let nextIndex: number | null = null;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (currentIndex + 1) % sections.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex = (currentIndex - 1 + sections.length) % sections.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = sections.length - 1;
    }
    if (nextIndex === null) return;

    event.preventDefault();
    const next = sections[nextIndex];
    if (next === undefined) return;
    setSection(next.id);
    queueMicrotask(() => document.getElementById(`settings-tab-${next.id}`)?.focus());
  };

  return (
    <main className="settings-shell">
      <aside className="settings-sidebar">
        <div className="settings-brand"><BrandMark small /><span>DryMark</span></div>
        <nav aria-label="Settings sections" role="tablist">
          {sections.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              id={`settings-tab-${id}`}
              type="button"
              role="tab"
              className={section === id ? "active" : ""}
              aria-controls={`settings-panel-${id}`}
              aria-selected={section === id}
              tabIndex={section === id ? 0 : -1}
              onClick={() => {
                setSection(id);
              }}
              onKeyDown={(event) => {
                moveSection(event, id);
              }}
            >
              <Icon size={17} /> {label}
            </button>
          ))}
        </nav>
        <div className="sidebar-privacy"><ShieldCheck size={15} /><span>Local processing<br />No app network access</span></div>
      </aside>

      <section
        className="settings-content"
        id={`settings-panel-${section}`}
        role="tabpanel"
        aria-labelledby={`settings-tab-${section}`}
        tabIndex={0}
      >
        {state.preferencesStatus === "write_failed" ? <PersistenceWarning /> : null}
        {section === "general" && <GeneralSettings state={state} bridge={bridge} />}
        {section === "cleaning" && <CleaningSettings state={state} bridge={bridge} />}
        {section === "privacy" && <PrivacySettings />}
        {section === "about" && <AboutSettings version={state.version} />}
      </section>
    </main>
  );
}

function PersistenceWarning() {
  return (
    <p className="persistence-warning">
      <CircleAlert size={14} /> Settings couldn’t be saved. Current session only.
    </p>
  );
}

function SettingsHeading({ title, description }: { title: string; description: string }) {
  return (
    <header className="settings-heading">
      <h1>{title}</h1>
      <p>{description}</p>
    </header>
  );
}

function GeneralSettings({ state, bridge }: { state: DryMarkState; bridge: DesktopBridge }) {
  return (
    <>
      <SettingsHeading title="General" description="Shortcut and feedback preferences." />
      <div className="settings-stack">
        <SettingCard icon={<ScanText size={18} />} title="Remove watermarks" description="Remove hidden watermark channels from the current clipboard without leaving Settings.">
          <button className="secondary-action" type="button" onClick={() => void bridge.cleanClipboard()}>
            <ShieldCheck size={15} /> Remove watermarks now
          </button>
        </SettingCard>
        <SettingCard icon={<Keyboard size={18} />} title="Global shortcut" description="Works while another app is focused.">
          <ShortcutRecorder shortcut={state.shortcutDisplay} onSave={(value) => void bridge.setShortcut(value)} />
          <StatusPill status={state.shortcutStatus} />
        </SettingCard>
        <SettingCard icon={<Sparkles size={18} />} title="Visual feedback" description="Show a silent confirmation after each removal.">
          <Switch
            checked={state.visualFeedback}
            label="Show silent visual feedback"
            onChange={(enabled) => void bridge.setVisualFeedback(enabled)}
          />
        </SettingCard>
        <div className="privacy-note">
          <ShieldCheck size={19} />
          <div><strong>Private by design</strong><p>DryMark never uploads or stores clipboard text. Reports contain counts only.</p></div>
        </div>
      </div>
    </>
  );
}

function CleaningSettings({ state, bridge }: { state: DryMarkState; bridge: DesktopBridge }) {
  return (
    <>
      <SettingsHeading title="Watermark removal" description="Choose how aggressively hidden LLM watermark channels are removed." />
      <div className="settings-stack">
        <PolicyPicker policy={state.policy} onChange={(policy) => void bridge.setPolicy(policy)} />
        <article className="policy-detail">
          <div className="policy-detail-icon"><FileKey2 size={19} /></div>
          <div>
            <h2>Preserve appearance</h2>
            <p>Minimizes presentation changes by retaining recognized emoji, script shaping, and balanced bidirectional text. Direction marks, annotation delimiters, and invisible notation operators are still removed, so specialist text can change.</p>
          </div>
        </article>
        <article className="policy-detail warning">
          <div className="policy-detail-icon"><CircleAlert size={19} /></div>
          <div>
            <h2>Thorough</h2>
            <p>Removes every format/default-ignorable channel and normalizes text. This can change emoji presentation, RTL layout, or specialist notation.</p>
          </div>
        </article>
        <div className="boundary-callout">
          <strong>Honest boundary</strong>
          <p>DryMark removes inspectable hidden LLM watermark channels. No lossless policy can remove every invisible channel while guaranteeing identical rendering or machine interpretation. Signals carried by visible wording, punctuation, sentence order, or semantics require rewriting the text.</p>
        </div>
      </div>
    </>
  );
}

function PrivacySettings() {
  return (
    <>
      <SettingsHeading title="Privacy" description="DryMark itself processes clipboard text locally." />
      <div className="settings-stack privacy-grid">
        {[
          ["No app network", "DryMark makes no network requests. Operating-system clipboard services may still sync clipboard data when enabled."],
          ["No app history", "DryMark never stores clipboard text in settings, logs, analytics, or reports."],
          ["Race aware", "Text is re-read before writing and verified after. OS clipboards are not atomic, so check any state-unknown warning."],
          ["Plain text", "Watermark removal intentionally replaces rich clipboard formats with one fresh text value."],
        ].map(([title, body]) => (
          <article className="privacy-card" key={title}>
            <ShieldCheck size={18} /><h2>{title}</h2><p>{body}</p>
          </article>
        ))}
      </div>
    </>
  );
}

function AboutSettings({ version }: { version: string }) {
  return (
    <>
      <SettingsHeading title="About" description="Local LLM watermark removal you can inspect." />
      <article className="about-card">
        <BrandMark />
        <h2>DryMark</h2>
        <p>Remove hidden watermarks from LLM text.</p>
        <span>Version {version}</span>
        <span>MIT licensed · Open source</span>
      </article>
    </>
  );
}

function SettingCard({
  icon,
  title,
  description,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <article className="setting-card">
      <div className="setting-copy"><span className="setting-icon">{icon}</span><div><h2>{title}</h2><p>{description}</p></div></div>
      <div className="setting-control">{children}</div>
    </article>
  );
}

function ShortcutRecorder({ shortcut, onSave }: { shortcut: string; onSave: (value: string) => void }) {
  const [recording, setRecording] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const start = () => {
    setRecording(true);
    queueMicrotask(() => buttonRef.current?.focus());
  };

  const capture = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    event.preventDefault();
    if (event.key === "Escape") {
      setRecording(false);
      return;
    }
    const value = shortcutFromEvent(event);
    if (value !== null) {
      setRecording(false);
      onSave(value);
    }
  };

  return (
    <button
      ref={buttonRef}
      className={`shortcut-recorder ${recording ? "recording" : ""}`}
      type="button"
      aria-label="Record a new shortcut"
      onClick={start}
      onKeyDown={capture}
    >
      <Keyboard size={15} /> {recording ? "Press shortcut…" : shortcut}
    </button>
  );
}

function shortcutFromEvent(event: KeyboardEvent): string | null {
  if (["Alt", "Shift", "Control", "Meta"].includes(event.key)) return null;
  if (!event.altKey && !event.shiftKey && !event.ctrlKey && !event.metaKey) return null;
  const parts: string[] = [];
  if (event.metaKey) parts.push("Super");
  if (event.ctrlKey) parts.push("Control");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  const key = event.code.startsWith("Key") ? event.code.slice(3) : event.key.toUpperCase();
  parts.push(key);
  return parts.join("+");
}

function Switch({ checked, label, onChange }: { checked: boolean; label: string; onChange: (checked: boolean) => void }) {
  return (
    <label className="switch-control">
      <span className="sr-only">{label}</span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      <span className="switch-track"><span /></span>
    </label>
  );
}

function StatusPill({ status }: { status: DryMarkState["shortcutStatus"] }) {
  return <span className={`status-pill ${status}`}>{shortcutStatusLabel(status)}</span>;
}

function ToastView({ state, bridge }: { state: DryMarkState; bridge: DesktopBridge }) {
  const copy = toastCopy(state.lastResult);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void bridge.closeCurrentWindow();
    }, 2400);
    return () => {
      window.clearTimeout(timer);
    };
  }, [bridge, state.lastResult]);

  return (
    <main
      className={`toast-shell ${copy.tone}`}
      role="status"
      aria-live="polite"
      aria-atomic="true"
    >
      <span className="toast-icon">{copy.tone === "success" ? <Check size={20} /> : <CircleAlert size={20} />}</span>
      <div><strong>{copy.title}</strong><p>{copy.detail}</p></div>
    </main>
  );
}

function toastCopy(result: LastResult | null) {
  if (result === null) return { title: "DryMark ready", detail: "Copy text, then press your shortcut.", tone: "success" } as const;
  if (result.kind === "cleaned") {
    return {
      title: "Watermark channels removed",
      detail: cleanedSummary(result),
      tone: "success",
    } as const;
  }
  if (result.kind === "already_clean") {
    const observed = result.observed > 0 ? ` ${String(result.observed)} contextual ${result.observed === 1 ? "character was" : "characters were"} preserved.` : "";
    return { title: "Already clean", detail: `No removable watermark channels were found.${observed}`, tone: "success" } as const;
  }
  if (result.kind === "clipboard_changed") return { title: "Clipboard changed", detail: "Nothing was overwritten. Try the shortcut again.", tone: "warning" } as const;
  if (result.kind === "empty") return { title: "Clipboard is empty", detail: "Copy some text and try again.", tone: "warning" } as const;
  if (result.kind === "non_text") return { title: "No text found", detail: "DryMark leaves non-text clipboard content untouched.", tone: "warning" } as const;
  if (result.kind === "too_large") return { title: "Text is too large", detail: "Nothing was overwritten. The limit is 16 MiB.", tone: "warning" } as const;
  if (result.kind === "write_failed") return { title: "Clipboard write failed", detail: "Clipboard state may have changed. Check it before pasting.", tone: "warning" } as const;
  if (result.kind === "write_unverified") return { title: "Couldn’t verify clipboard", detail: "Clipboard state may have changed. Check it before pasting.", tone: "warning" } as const;
  return { title: "Couldn’t read clipboard", detail: "Nothing was overwritten. Check access and try again.", tone: "warning" } as const;
}

function cleanedSummary(result: LastResult) {
  const parts: string[] = [];
  if (result.removed > 0) {
    parts.push(`${String(result.removed)} hidden ${result.removed === 1 ? "character" : "characters"} removed`);
  }
  if (result.observed > 0) {
    parts.push(`${String(result.observed)} contextual ${result.observed === 1 ? "character" : "characters"} preserved`);
  }
  if (result.canonicalized) parts.push("text canonicalized");
  if (result.formattingCleared) parts.push("formatting cleared");
  return parts.length > 0 ? parts.join(" · ") : "Plain text refreshed";
}

function shortcutStatusLabel(status: DryMarkState["shortcutStatus"]) {
  return ({
    registered: "Ready",
    conflict: "Shortcut conflict",
    unsupported: "Shortcut unavailable",
    permission_denied: "Permission needed",
    invalid: "Invalid shortcut",
  } as const)[status];
}

function lastResultTitle(result: LastResult | null) {
  if (result === null) return "Waiting for first removal";
  if (result.kind === "cleaned") return "Watermark channels removed";
  if (result.kind === "already_clean") return "Already clean";
  if (result.kind === "clipboard_changed") return "Clipboard changed safely";
  if (result.kind === "write_failed" || result.kind === "write_unverified") return "Clipboard state unknown";
  return "Clipboard left untouched";
}

function lastResultTone(result: LastResult | null) {
  if (result === null) return "neutral";
  return result.kind === "cleaned" || result.kind === "already_clean" ? "success" : "warning";
}

function lastResultIcon(result: LastResult | null) {
  if (result === null) return <ScanText size={18} />;
  return result.kind === "cleaned" || result.kind === "already_clean"
    ? <Check size={18} />
    : <CircleAlert size={18} />;
}

function lastResultDetail(result: LastResult | null) {
  if (result === null) return "No clipboard text processed yet";
  if (result.kind === "cleaned") return cleanedSummary(result);
  if (result.kind === "already_clean") {
    return result.observed > 0
      ? `No removable watermark channels; ${String(result.observed)} contextual ${result.observed === 1 ? "character" : "characters"} preserved`
      : "No removable watermark channels found";
  }
  if (result.kind === "clipboard_changed") return "Clipboard changed before the write";
  if (result.kind === "empty") return "Clipboard text was empty";
  if (result.kind === "non_text") return "Clipboard did not contain text";
  if (result.kind === "too_large") return "Clipboard text exceeded 16 MiB";
  if (result.kind === "write_failed") return "The plain-text write did not complete";
  if (result.kind === "write_unverified") return "The written text could not be verified";
  return "The operation failed safely";
}

function lastResultClipboardState(result: LastResult | null) {
  if (result?.formattingCleared) return "Formatting cleared";
  if (result?.kind === "already_clean") return "Plain text verified";
  if (result?.kind === "write_failed" || result?.kind === "write_unverified") return "Clipboard state unknown";
  return "No clipboard write";
}
