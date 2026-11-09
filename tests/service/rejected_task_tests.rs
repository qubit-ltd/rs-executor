use std::error::Error;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use qubit_executor::{RejectedTask, SubmissionError};

struct NoDebugTask;

#[test]
fn test_rejected_callable_can_be_recovered() {
    let task = move || Ok::<usize, ()>(42);
    let rejected = RejectedTask::new(SubmissionError::Saturated, task);
    assert_eq!(rejected.reason(), &SubmissionError::Saturated);
    let (reason, task) = rejected.into_parts();
    assert_eq!(reason, SubmissionError::Saturated);
    assert_eq!(task(), Ok(42));
}

#[test]
fn test_formatting_does_not_require_or_display_task() {
    fn assert_debug<T: Debug>(value: &T) -> String {
        format!("{value:?}")
    }

    let rejected = RejectedTask::new(SubmissionError::Shutdown, NoDebugTask);
    let display = rejected.to_string();
    let debug = assert_debug(&rejected);
    assert!(display.contains("shut down"));
    assert!(debug.contains("Shutdown"));
    assert!(!debug.contains("NoDebugTask"));
}

#[test]
fn test_source_is_submission_error_and_reason_is_formatted() {
    let rejected = RejectedTask::new(SubmissionError::Saturated, NoDebugTask);
    let source = Error::source(&rejected).expect("submission error source");
    assert!(source.downcast_ref::<SubmissionError>().is_some());
    assert_eq!(source.to_string(), SubmissionError::Saturated.to_string());
    assert!(rejected.to_string().contains("saturated"));
}

#[test]
fn test_drop_and_into_parts_preserve_captured_state() {
    struct DropProbe(Arc<AtomicUsize>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let captured = DropProbe(Arc::clone(&drops));
    let task = move || {
        let _ = &captured;
        Ok::<usize, ()>(42)
    };
    let rejected = RejectedTask::new(SubmissionError::Saturated, task);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let (reason, task) = rejected.into_parts();
    assert_eq!(reason, SubmissionError::Saturated);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(task(), Ok(42));
    drop(task);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn test_dropping_rejected_task_drops_callable_once() {
    struct DropProbe(Arc<AtomicUsize>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let captured = DropProbe(Arc::clone(&drops));
    let task = move || {
        let _ = &captured;
        Ok::<(), ()>(())
    };
    drop(RejectedTask::new(SubmissionError::Shutdown, task));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
