#![allow(missing_docs)]

use std::collections::VecDeque;

use drymark_core::Policy;
use drymark_transaction::{
    CleanOutcome, ClipboardError, ClipboardPort, ClipboardSnapshot, Coordinator,
};

#[derive(Default)]
struct MockClipboard {
    reads: VecDeque<Result<Option<ClipboardSnapshot>, ClipboardError>>,
    writes: Vec<String>,
    write_error: Option<ClipboardError>,
}

impl ClipboardPort for MockClipboard {
    fn read_text(&mut self) -> Result<Option<ClipboardSnapshot>, ClipboardError> {
        self.reads
            .pop_front()
            .unwrap_or(Err(ClipboardError::Unavailable))
    }

    fn replace_with_plain_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if let Some(error) = self.write_error {
            return Err(error);
        }
        self.writes.push(text.to_owned());
        Ok(())
    }
}

fn snapshot(text: &str, revision: Option<u64>, rewrite_required: bool) -> ClipboardSnapshot {
    ClipboardSnapshot::new(text.to_owned(), revision, rewrite_required)
}

#[test]
fn sanitizes_after_a_matching_second_read() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("same\u{200b} words", Some(7), true))),
            Ok(Some(snapshot("same\u{200b} words", Some(7), true))),
            Ok(Some(snapshot("same words", Some(8), false))),
        ]),
        ..MockClipboard::default()
    };

    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    assert!(matches!(outcome, CleanOutcome::Cleaned { .. }));
    assert_eq!(clipboard.writes, ["same words"]);
}

#[test]
fn leaves_an_already_clean_plain_clipboard_untouched() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([Ok(Some(snapshot("plain", Some(1), false)))]),
        ..MockClipboard::default()
    };

    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    assert!(matches!(outcome, CleanOutcome::AlreadyClean { .. }));
    assert!(clipboard.writes.is_empty());
    assert!(
        clipboard.reads.is_empty(),
        "no race read is needed without a write"
    );
}

#[test]
fn rewrites_clean_text_when_extra_clipboard_formats_exist() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("plain", Some(3), true))),
            Ok(Some(snapshot("plain", Some(3), true))),
            Ok(Some(snapshot("plain", Some(4), false))),
        ]),
        ..MockClipboard::default()
    };

    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    assert!(matches!(outcome, CleanOutcome::Cleaned { .. }));
    assert_eq!(clipboard.writes, ["plain"]);
}

#[test]
fn rewrites_empty_text_when_hidden_clipboard_formats_may_exist() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("", Some(3), true))),
            Ok(Some(snapshot("", Some(3), true))),
            Ok(Some(snapshot("", Some(4), false))),
        ]),
        ..MockClipboard::default()
    };

    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    assert!(matches!(outcome, CleanOutcome::Cleaned { .. }));
    assert_eq!(clipboard.writes, [""]);
}

#[test]
fn aborts_if_text_changes_during_sanitation() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("first\u{200b}", None, false))),
            Ok(Some(snapshot("second", None, false))),
        ]),
        ..MockClipboard::default()
    };

    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    assert_eq!(outcome, CleanOutcome::ClipboardChanged);
    assert!(clipboard.writes.is_empty());
}

#[test]
fn aborts_if_revision_changes_even_when_text_is_identical() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("same\u{200b}", Some(8), false))),
            Ok(Some(snapshot("same\u{200b}", Some(9), false))),
        ]),
        ..MockClipboard::default()
    };

    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    assert_eq!(outcome, CleanOutcome::ClipboardChanged);
    assert!(clipboard.writes.is_empty());
}

#[test]
fn aborts_if_revision_metadata_appears_or_disappears() {
    for revisions in [(Some(8), None), (None, Some(8))] {
        let mut clipboard = MockClipboard {
            reads: VecDeque::from([
                Ok(Some(snapshot("same\u{200b}", revisions.0, false))),
                Ok(Some(snapshot("same\u{200b}", revisions.1, false))),
            ]),
            ..MockClipboard::default()
        };

        let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
        assert_eq!(outcome, CleanOutcome::ClipboardChanged);
        assert!(clipboard.writes.is_empty());
    }
}

#[test]
fn aborts_if_the_adapter_rewrite_requirement_changes() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("same\u{200b}", None, true))),
            Ok(Some(snapshot("same\u{200b}", None, false))),
        ]),
        ..MockClipboard::default()
    };

    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    assert_eq!(outcome, CleanOutcome::ClipboardChanged);
    assert!(clipboard.writes.is_empty());
}

#[test]
fn rejects_oversized_text_before_a_second_read_or_write() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([Ok(Some(snapshot("12345", Some(1), false)))]),
        ..MockClipboard::default()
    };
    let coordinator = Coordinator::new(4);

    assert_eq!(
        coordinator.clean(&mut clipboard, Policy::PreserveAppearance),
        CleanOutcome::TooLarge { limit_bytes: 4 }
    );
    assert!(clipboard.reads.is_empty());
    assert!(clipboard.writes.is_empty());
}

#[test]
fn handles_empty_non_text_and_read_failures_without_writes() {
    let cases = [
        (Ok(None), CleanOutcome::NonText),
        (Ok(Some(snapshot("", Some(1), false))), CleanOutcome::Empty),
        (
            Err(ClipboardError::Busy),
            CleanOutcome::ReadFailed {
                error: ClipboardError::Busy,
            },
        ),
        (
            Err(ClipboardError::PermissionDenied),
            CleanOutcome::ReadFailed {
                error: ClipboardError::PermissionDenied,
            },
        ),
    ];

    for (read, expected) in cases {
        let mut clipboard = MockClipboard {
            reads: VecDeque::from([read]),
            ..MockClipboard::default()
        };
        assert_eq!(
            Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance),
            expected
        );
        assert!(clipboard.writes.is_empty());
    }
}

#[test]
fn write_failures_are_classified_without_claiming_success() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("a\u{200b}", Some(1), false))),
            Ok(Some(snapshot("a\u{200b}", Some(1), false))),
        ]),
        write_error: Some(ClipboardError::Busy),
        ..MockClipboard::default()
    };

    assert_eq!(
        Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance),
        CleanOutcome::WriteFailed {
            error: ClipboardError::Busy,
        }
    );
    assert!(clipboard.writes.is_empty());
}

#[test]
fn a_failed_post_write_read_never_claims_verified_success() {
    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(snapshot("a\u{200b}", Some(1), false))),
            Ok(Some(snapshot("a\u{200b}", Some(1), false))),
            Err(ClipboardError::Busy),
        ]),
        ..MockClipboard::default()
    };

    assert_eq!(
        Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance),
        CleanOutcome::WriteVerificationFailed {
            error: ClipboardError::Busy,
        }
    );
    assert_eq!(clipboard.writes, ["a"]);
}

#[test]
fn a_post_write_mismatch_reports_unknown_clipboard_state() {
    for final_read in [None, Some(snapshot("another value", Some(2), false))] {
        let mut clipboard = MockClipboard {
            reads: VecDeque::from([
                Ok(Some(snapshot("a\u{200b}", Some(1), false))),
                Ok(Some(snapshot("a\u{200b}", Some(1), false))),
                Ok(final_read),
            ]),
            ..MockClipboard::default()
        };

        assert_eq!(
            Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance),
            CleanOutcome::WriteVerificationMismatch
        );
        assert_eq!(clipboard.writes, ["a"]);
    }
}

#[test]
fn snapshots_and_outcomes_redact_clipboard_contents() -> Result<(), serde_json::Error> {
    let secret = "PRIVATE-ZXQ-9182\u{200b}";
    let redacted_snapshot = snapshot(secret, Some(99), true);
    assert!(!format!("{redacted_snapshot:?}").contains("PRIVATE"));

    let mut clipboard = MockClipboard {
        reads: VecDeque::from([
            Ok(Some(redacted_snapshot)),
            Ok(Some(snapshot(secret, Some(99), true))),
            Ok(Some(snapshot("PRIVATE-ZXQ-9182", Some(100), false))),
        ]),
        ..MockClipboard::default()
    };
    let outcome = Coordinator::default().clean(&mut clipboard, Policy::PreserveAppearance);
    let json = serde_json::to_string(&outcome)?;
    assert!(!json.contains("PRIVATE"));
    assert!(!json.contains("ZXQ-9182"));
    Ok(())
}
