use std::time::Duration;
use std::time::Instant;

use parking_lot::Condvar;
use parking_lot::Mutex;

use super::super::ExecutorServiceLifecycle;
use super::super::SubmissionError;

#[derive(Debug, Clone, Copy)]
struct ServiceState {
    lifecycle: ExecutorServiceLifecycle,
    active_tasks: usize,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self {
            lifecycle: ExecutorServiceLifecycle::Running,
            active_tasks: 0,
        }
    }
}

#[derive(Default)]
pub struct ThreadPerTaskExecutorServiceState {
    state: Mutex<ServiceState>,
    termination: Condvar,
}

impl ThreadPerTaskExecutorServiceState {
    #[inline]
    pub fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.state.lock().lifecycle
    }

    #[inline]
    pub fn accept_task(&self) -> Result<(), SubmissionError> {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Running {
            return Err(SubmissionError::Shutdown);
        }
        state.active_tasks += 1;
        Ok(())
    }

    #[inline]
    pub(crate) fn finish_task(&self) {
        let mut state = self.state.lock();
        state.active_tasks -= 1;
        Self::terminate_if_ready(&mut state, &self.termination);
    }

    pub fn wait_for_termination(&self) {
        let mut state = self.state.lock();
        while state.lifecycle != ExecutorServiceLifecycle::Terminated {
            self.termination.wait(&mut state);
        }
    }

    pub fn wait_for_termination_timeout(&self, timeout: Duration) -> bool {
        let started = Instant::now();
        let mut state = self.state.lock();
        loop {
            if state.lifecycle == ExecutorServiceLifecycle::Terminated {
                return true;
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return false;
            }
            self.termination
                .wait_for(&mut state, remaining.min(Duration::from_secs(3600)));
        }
    }

    #[inline]
    pub fn shutdown(&self) {
        let mut state = self.state.lock();
        if state.lifecycle == ExecutorServiceLifecycle::Running {
            state.lifecycle = ExecutorServiceLifecycle::ShuttingDown;
        }
        Self::terminate_if_ready(&mut state, &self.termination);
    }

    #[inline]
    pub fn stop(&self) -> usize {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Terminated {
            state.lifecycle = ExecutorServiceLifecycle::Stopping;
        }
        let running = state.active_tasks;
        Self::terminate_if_ready(&mut state, &self.termination);
        running
    }

    #[inline]
    fn terminate_if_ready(state: &mut ServiceState, termination: &Condvar) {
        if state.lifecycle != ExecutorServiceLifecycle::Running && state.active_tasks == 0 {
            state.lifecycle = ExecutorServiceLifecycle::Terminated;
            termination.notify_all();
        }
    }
}
