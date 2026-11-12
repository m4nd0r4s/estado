//! The machine is meant to be reused by BM-04 (10,000 machines times 60
//! events): this checks what that needs, without measuring anything yet.
//!
//! A counting global allocator, per thread so parallel tests do not disturb
//! it, proves that `send` allocates nothing once the machine is built.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use estado_core::{Discard, Machine};
use estado_examples::d_game_character::{Character, GameCtx};

use super::support::{Lcg, random_event};

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

fn allocations() -> u64 {
    ALLOCATIONS.with(Cell::get)
}

struct CountingAllocator;

fn count_one() {
    // `try_with`: the thread-local may already be gone while a thread exits.
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
}

// SAFETY: every method forwards to `System` unchanged; the counting touches
// only a thread-local `Cell` and never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_one();
        // SAFETY: same contract as ours, forwarded.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_one();
        // SAFETY: same contract as ours, forwarded.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: same contract as ours, forwarded.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_one();
        // SAFETY: same contract as ours, forwarded.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn the_counting_allocator_counts() {
    let before = allocations();
    let boxed = Box::new(7_u64);
    assert_eq!(allocations(), before + 1);
    drop(boxed);
}

#[test]
fn sending_events_never_allocates() {
    let mut machine = Machine::<Character>::new(GameCtx::untraced());
    machine.start().unwrap();
    let mut rng = Lcg::new(99);

    let before = allocations();
    // `send` builds the inline effect buffer on each call: if any step
    // needed more than its four slots it would spill to the heap and count.
    for _ in 0..20_000 {
        machine.send(random_event(&mut rng)).unwrap();
    }
    // `send_into` with a sink that discards, the path BM-04 will take.
    for _ in 0..20_000 {
        machine
            .send_into(random_event(&mut rng), &mut Discard)
            .unwrap();
    }
    assert_eq!(allocations() - before, 0);
}

#[test]
fn an_untraced_machine_does_not_allocate_for_its_context_either() {
    let before = allocations();
    let mut machines: Vec<Machine<Character>> = Vec::with_capacity(100);
    let setup = allocations() - before;
    assert_eq!(setup, 1, "only the vector itself");
    for _ in 0..100 {
        machines.push(Machine::new(GameCtx::untraced()));
    }
    for machine in &mut machines {
        machine.start().unwrap();
    }
    assert_eq!(allocations() - before, 1, "100 machines, no allocation");
}
