#![no_main]

use std::collections::VecDeque;

use libfuzzer_sys::fuzz_target;
use drymark_core::{Policy, sanitize};
use drymark_transaction::{
    CleanOutcome, ClipboardError, ClipboardPort, ClipboardSnapshot, Coordinator,
    DEFAULT_MAX_INPUT_BYTES,
};

#[derive(Default)]
struct Clipboard {
    reads: VecDeque<Result<Option<ClipboardSnapshot>, ClipboardError>>,
    writes: Vec<String>,
    write_error: Option<ClipboardError>,
}

impl ClipboardPort for Clipboard {
    fn read_text(&mut self) -> Result<Option<ClipboardSnapshot>, ClipboardError> {
        self.reads.pop_front().unwrap_or(Ok(None))
    }

    fn replace_with_plain_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if let Some(error) = self.write_error {
            return Err(error);
        }
        self.writes.push(text.to_owned());
        Ok(())
    }
}

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() < 2 {
        return;
    }
    let flags = bytes[0];
    let payload = &bytes[2..];
    let midpoint = usize::from(bytes[1]) % (payload.len() + 1);
    let first = String::from_utf8_lossy(&payload[..midpoint]);
    let second = String::from_utf8_lossy(&payload[midpoint..]);
    let policy = if flags & 1 == 0 {
        Policy::PreserveAppearance
    } else {
        Policy::Thorough
    };
    let scenario = (flags >> 3) & 7;
    let same_revision = flags & 2 == 0;
    let rewrite_required = flags & 4 != 0 || scenario >= 3;
    let expected = sanitize(first.as_ref(), policy).text;
    let first_snapshot = || {
        Ok(Some(ClipboardSnapshot::new(
            first.to_string(),
            Some(1),
            rewrite_required,
        )))
    };
    let matching_snapshot = || {
        Ok(Some(ClipboardSnapshot::new(
            first.to_string(),
            Some(1),
            rewrite_required,
        )))
    };
    let successful_verification = || {
        Ok(Some(ClipboardSnapshot::new(
            expected.clone(),
            Some(3),
            false,
        )))
    };
    let (reads, write_error) = match scenario {
        1 => (VecDeque::from([Err(ClipboardError::Busy)]), None),
        2 => (VecDeque::from([Ok(None)]), None),
        3 => (
            VecDeque::from([first_snapshot(), Err(ClipboardError::PermissionDenied)]),
            None,
        ),
        4 => (VecDeque::from([first_snapshot(), Ok(None)]), None),
        5 => (
            VecDeque::from([first_snapshot(), matching_snapshot()]),
            Some(ClipboardError::Busy),
        ),
        6 => (
            VecDeque::from([
                first_snapshot(),
                matching_snapshot(),
                Err(ClipboardError::Platform),
            ]),
            None,
        ),
        7 => (
            VecDeque::from([first_snapshot(), matching_snapshot(), Ok(None)]),
            None,
        ),
        _ => (
            VecDeque::from([
                first_snapshot(),
                Ok(Some(ClipboardSnapshot::new(
                    second.to_string(),
                    Some(if same_revision { 1 } else { 2 }),
                    rewrite_required,
                ))),
                successful_verification(),
            ]),
            None,
        ),
    };
    let mut clipboard = Clipboard {
        reads,
        writes: Vec::new(),
        write_error,
    };

    let outcome = Coordinator::default().clean(&mut clipboard, policy);
    if scenario == 1 {
        assert!(matches!(outcome, CleanOutcome::ReadFailed { .. }));
        assert!(clipboard.writes.is_empty());
        return;
    }
    if scenario == 2 {
        assert_eq!(outcome, CleanOutcome::NonText);
        assert!(clipboard.writes.is_empty());
        return;
    }
    if first.is_empty() && !rewrite_required {
        assert_eq!(outcome, CleanOutcome::Empty);
        assert!(clipboard.writes.is_empty());
        return;
    }
    if first.len() > DEFAULT_MAX_INPUT_BYTES {
        assert!(matches!(outcome, CleanOutcome::TooLarge { .. }));
        assert!(clipboard.writes.is_empty());
        return;
    }
    match scenario {
        3 => assert!(matches!(outcome, CleanOutcome::RecheckFailed { .. })),
        4 => assert_eq!(outcome, CleanOutcome::ClipboardChanged),
        5 => assert!(matches!(outcome, CleanOutcome::WriteFailed { .. })),
        6 => assert!(matches!(
            outcome,
            CleanOutcome::WriteVerificationFailed { .. }
        )),
        7 => assert_eq!(outcome, CleanOutcome::WriteVerificationMismatch),
        _ => {}
    }
    if scenario != 0 {
        assert_eq!(clipboard.writes.is_empty(), scenario <= 5);
        return;
    }

    let may_write =
        first == second && same_revision && (expected != first.as_ref() || rewrite_required);

    assert_eq!(!clipboard.writes.is_empty(), may_write);
    if let Some(written) = clipboard.writes.first() {
        assert_eq!(written, &expected);
        assert!(matches!(outcome, CleanOutcome::Cleaned { .. }));
    } else {
        assert!(!matches!(outcome, CleanOutcome::Cleaned { .. }));
    }
});
