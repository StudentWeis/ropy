//! Exercise the patched legacy Blocks ABI against macOS's native runtime.
#![cfg(target_os = "macos")]

use std::{cell::Cell, rc::Rc};

use block::ConcreteBlock;

#[test]
fn test_block_stack_callback_passes_arguments_and_returns_result() {
    let block = ConcreteBlock::new(|left: i32, right: i32| left + right);
    // SAFETY: this block and its typed callback were created here and stay alive
    // for the call; no Objective-C objects or shared mutable state are involved.
    assert_eq!(unsafe { block.call((17, 25)) }, 42);
}

struct DropProbe(Rc<Cell<usize>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn test_block_heap_copy_survives_stack_and_releases_capture_once() {
    let drops = Rc::new(Cell::new(0));
    let block = {
        let probe = DropProbe(drops.clone());
        ConcreteBlock::new(move |value: i32| {
            let _keep_alive = &probe;
            value + 5
        })
        .copy()
    };
    let retained = block.clone();
    drop(block);
    assert_eq!(drops.get(), 0);
    // SAFETY: the retained native block owns the captured closure, the callback
    // signature matches, and all calls and releases occur on this thread.
    assert_eq!(unsafe { retained.call((37,)) }, 42);
    drop(retained);
    assert_eq!(drops.get(), 1);
}
