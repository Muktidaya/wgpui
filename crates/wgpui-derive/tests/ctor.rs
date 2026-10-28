use std::sync::atomic::{AtomicBool, Ordering};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static UNSAFE_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[wgpui::ctor(unsafe)]
fn initialize() {
    INITIALIZED.store(true, Ordering::SeqCst);
}

#[wgpui::ctor]
unsafe fn initialize_unsafe() {
    UNSAFE_INITIALIZED.store(true, Ordering::SeqCst);
}

#[test]
fn constructor_runs_without_a_direct_ctor_dependency() {
    assert!(INITIALIZED.load(Ordering::SeqCst));
    assert!(UNSAFE_INITIALIZED.load(Ordering::SeqCst));
}
