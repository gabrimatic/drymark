//! Race-aware clipboard sanitation coordination.
//!
//! The coordinator owns no platform APIs. Native adapters implement
//! [`ClipboardPort`], while this crate enforces size limits, a second-read race
//! check, plain-text replacement, immediate post-write text verification, and
//! metadata-only outcomes.

use std::fmt;

use drymark_core::{Policy, SanitizeReport, sanitize};
use serde::Serialize;
use zeroize::Zeroizing;

/// Default maximum clipboard text size: 16 MiB.
pub const DEFAULT_MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

/// A portable, non-sensitive clipboard failure classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardError {
    /// Another process temporarily owns or locks the clipboard.
    Busy,
    /// Operating-system privacy controls denied access.
    PermissionDenied,
    /// The required clipboard surface is unavailable.
    Unavailable,
    /// The platform returned data that could not be represented as UTF-8 text.
    InvalidText,
    /// An uncategorized platform operation failed.
    Platform,
}

/// A sensitive clipboard read plus non-sensitive revision metadata.
///
/// Debug output is redacted and the owned text is zeroed when dropped.
pub struct ClipboardSnapshot {
    text: Zeroizing<String>,
    revision: Option<u64>,
    rewrite_required: bool,
}

impl ClipboardSnapshot {
    /// Construct a snapshot from platform clipboard data.
    ///
    /// `rewrite_required` is true when extra formats are known to exist or the
    /// adapter cannot enumerate formats and must conservatively rewrite.
    #[must_use]
    pub fn new(text: String, revision: Option<u64>, rewrite_required: bool) -> Self {
        Self {
            text: Zeroizing::new(text),
            revision,
            rewrite_required,
        }
    }

    fn text(&self) -> &str {
        self.text.as_str()
    }
}

impl fmt::Debug for ClipboardSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClipboardSnapshot")
            .field("text", &"[REDACTED]")
            .field("text_bytes", &self.text.len())
            .field("revision", &self.revision)
            .field("rewrite_required", &self.rewrite_required)
            .finish()
    }
}

/// Platform clipboard operations needed by the coordinator.
pub trait ClipboardPort {
    /// Read plain text and any available revision/format metadata.
    ///
    /// # Errors
    ///
    /// Returns a portable [`ClipboardError`] when the platform read cannot be
    /// completed. Implementations must not include clipboard content in the
    /// error value.
    fn read_text(&mut self) -> Result<Option<ClipboardSnapshot>, ClipboardError>;

    /// Replace existing representations with one fresh plain-text value.
    ///
    /// # Errors
    ///
    /// Returns a portable [`ClipboardError`] when the platform cannot replace
    /// the clipboard. A failed call may leave clipboard state unknown; callers
    /// must not assume the prior contents remain intact. Implementations must
    /// leave error reporting free of text excerpts.
    fn replace_with_plain_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}

/// Metadata-only result of one clipboard sanitation attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CleanOutcome {
    /// Text and/or extra formats were replaced with sanitized plain text.
    Cleaned {
        /// Unicode sanitation report.
        report: SanitizeReport,
    },
    /// Clipboard text was already clean and had no extra representations.
    AlreadyClean {
        /// Unicode sanitation report.
        report: SanitizeReport,
    },
    /// Clipboard contained no transferable representation.
    NonText,
    /// Clipboard contained an empty text representation with no rewrite needed.
    Empty,
    /// Clipboard text exceeded the configured size safety limit.
    TooLarge {
        /// Configured byte limit.
        limit_bytes: usize,
    },
    /// Clipboard contents changed between the first read and intended write.
    ClipboardChanged,
    /// The initial clipboard read failed.
    ReadFailed {
        /// Portable failure classification.
        error: ClipboardError,
    },
    /// The safety re-read failed; nothing was written.
    RecheckFailed {
        /// Portable failure classification.
        error: ClipboardError,
    },
    /// The final plain-text write failed.
    WriteFailed {
        /// Portable failure classification.
        error: ClipboardError,
    },
    /// The write returned success, but the immediate verification read failed.
    WriteVerificationFailed {
        /// Portable failure classification.
        error: ClipboardError,
    },
    /// The immediate post-write clipboard value did not match the intended text.
    WriteVerificationMismatch,
}

/// Stateless clipboard sanitation coordinator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Coordinator {
    max_input_bytes: usize,
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_INPUT_BYTES)
    }
}

impl Coordinator {
    /// Construct a coordinator with an explicit UTF-8 byte limit.
    #[must_use]
    pub const fn new(max_input_bytes: usize) -> Self {
        Self { max_input_bytes }
    }

    /// Clean a clipboard transactionally under the selected policy.
    pub fn clean<P: ClipboardPort>(&self, clipboard: &mut P, policy: Policy) -> CleanOutcome {
        let first = match clipboard.read_text() {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => return CleanOutcome::NonText,
            Err(error) => return CleanOutcome::ReadFailed { error },
        };

        if first.text().is_empty() && !first.rewrite_required {
            return CleanOutcome::Empty;
        }
        if first.text.len() > self.max_input_bytes {
            return CleanOutcome::TooLarge {
                limit_bytes: self.max_input_bytes,
            };
        }

        let sanitized = sanitize(first.text(), policy);
        let sanitized_text = Zeroizing::new(sanitized.text);
        if !sanitized.report.changed && !first.rewrite_required {
            return CleanOutcome::AlreadyClean {
                report: sanitized.report,
            };
        }

        let second = match clipboard.read_text() {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => return CleanOutcome::ClipboardChanged,
            Err(error) => return CleanOutcome::RecheckFailed { error },
        };

        if !same_clipboard_value(&first, &second) {
            return CleanOutcome::ClipboardChanged;
        }

        if let Err(error) = clipboard.replace_with_plain_text(sanitized_text.as_str()) {
            return CleanOutcome::WriteFailed { error };
        }

        match clipboard.read_text() {
            Ok(Some(snapshot)) if snapshot.text() == sanitized_text.as_str() => {
                CleanOutcome::Cleaned {
                    report: sanitized.report,
                }
            }
            Ok(_) => CleanOutcome::WriteVerificationMismatch,
            Err(error) => CleanOutcome::WriteVerificationFailed { error },
        }
    }
}

fn same_clipboard_value(first: &ClipboardSnapshot, second: &ClipboardSnapshot) -> bool {
    first.revision == second.revision
        && first.text() == second.text()
        && first.rewrite_required == second.rewrite_required
}
