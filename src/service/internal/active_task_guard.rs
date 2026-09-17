use std::sync::Arc;

use super::thread_per_task_executor_service_state::ThreadPerTaskExecutorServiceState;

pub struct ActiveTaskGuard {
    state: Arc<ThreadPerTaskExecutorServiceState>,
}

impl ActiveTaskGuard {
    #[inline]
    pub fn new(state: Arc<ThreadPerTaskExecutorServiceState>) -> Self {
        Self { state }
    }
}

impl Drop for ActiveTaskGuard {
    #[inline]
    fn drop(&mut self) {
        self.state.finish_task();
    }
}
