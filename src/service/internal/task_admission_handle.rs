use crate::TrackedTask;
use crate::task::TaskHandle;

pub trait TaskAdmissionHandle {
    fn mark_accepted(&self);
}

impl<R, E> TaskAdmissionHandle for TaskHandle<R, E> {
    #[inline]
    fn mark_accepted(&self) {
        self.accept();
    }
}

impl<R, E> TaskAdmissionHandle for TrackedTask<R, E> {
    #[inline]
    fn mark_accepted(&self) {
        self.accept();
    }
}
