#!/usr/bin/env python3
"""Compile the production set_display method against a deterministic queued executor.

No Win32 calls or GPUI dependency build is needed. This checks scheduling and
callback restoration, not Windows message delivery or the real executor.
Set EVENTS_SOURCE to test a baseline copy of events.rs.
"""
import os
from pathlib import Path
import subprocess
import tempfile

source = Path(os.environ.get("EVENTS_SOURCE", Path(__file__).resolve().parents[1] / "src/events.rs")).read_text()
start = source.index("    fn set_display(")
end = source.index("    fn handle_hit_test_msg", start)
method = source[start:end]
harness = r'''
use std::{cell::{Cell, RefCell}, collections::VecDeque, future::Future, pin::Pin,
    rc::Rc, sync::Arc, task::{Context, Poll, Wake, Waker}};
type WindowsDisplay = u32;
type Callback = Box<dyn FnMut()>;
type Work = Pin<Box<dyn Future<Output = ()>>>;
#[derive(Clone, Default)]
struct Executor(Rc<RefCell<VecDeque<Work>>>);
struct Task;
impl Task { fn detach(self) {} }
impl Executor {
    fn spawn(&self, work: impl Future<Output = ()> + 'static) -> Task {
        self.0.borrow_mut().push_back(Box::pin(work));
        Task
    }
    fn drain(&self) {
        struct Noop;
        impl Wake for Noop { fn wake(self: Arc<Self>) {} }
        let waker = Waker::from(Arc::new(Noop));
        let mut cx = Context::from_waker(&waker);
        loop {
            let Some(mut work) = self.0.borrow_mut().pop_front() else { break; };
            assert_eq!(work.as_mut().poll(&mut cx), Poll::Ready(()));
        }
    }
}
struct Callbacks { display_changed: Cell<Option<Callback>> }
struct State { display: Cell<WindowsDisplay>, callbacks: Callbacks }
struct WindowsWindowInner { state: State, executor: Executor }
impl WindowsWindowInner {
__METHOD__
}
fn window(executor: Executor, callback: impl FnMut() + 'static) -> Rc<WindowsWindowInner> {
    Rc::new(WindowsWindowInner {
        state: State { display: Cell::new(0), callbacks: Callbacks {
            display_changed: Cell::new(Some(Box::new(callback))) } }, executor,
    })
}
#[test]
fn does_not_reenter_update_and_restores_callback_for_next_message() {
    let executor = Executor::default();
    let update = Rc::new(RefCell::new(()));
    let callback_update = update.clone();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let window = window(executor.clone(), move || {
        let _guard = callback_update.borrow_mut();
        observed.set(observed.get() + 1);
    });
    let guard = update.borrow_mut();
    window.set_display(1);
    assert_eq!(window.state.display.get(), 1);
    assert_eq!(calls.get(), 0);
    drop(guard);
    executor.drain();
    assert_eq!(calls.get(), 1);
    window.set_display(2);
    executor.drain();
    assert_eq!(calls.get(), 2);
}
#[test]
fn queued_notification_retains_state_until_dispatch() {
    let executor = Executor::default();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let window = window(executor.clone(), move || observed.set(observed.get() + 1));
    let weak = Rc::downgrade(&window);
    window.set_display(1);
    drop(window);
    assert!(weak.upgrade().is_some());
    executor.drain();
    assert_eq!(calls.get(), 1);
    assert!(weak.upgrade().is_none());
}
'''.replace("__METHOD__", method)
with tempfile.TemporaryDirectory(prefix="araview-display-test-") as temporary:
    root = Path(temporary)
    test_source = root / "display_change.rs"
    test_source.write_text(harness)
    binary = root / ("display_change.exe" if os.name == "nt" else "display_change")
    subprocess.run(["rustc", "--edition", "2024", "--test", str(test_source), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
