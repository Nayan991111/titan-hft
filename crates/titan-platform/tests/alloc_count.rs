use titan_platform::alloc_count::{thread_allocations, CountingAlloc};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

#[test]
fn counts_allocations_on_this_thread() {
    let before = thread_allocations();
    let v: Vec<u64> = Vec::with_capacity(1024);
    std::hint::black_box(&v);
    assert!(thread_allocations() > before);
}

#[test]
fn preallocated_hot_loop_allocates_nothing() {
    let mut buf = vec![0u64; 4096];
    let before = thread_allocations();
    let mut acc = 0u64;
    for i in 0..10_000_000u64 {
        let slot = &mut buf[(i as usize) & 4095];
        *slot = slot.wrapping_add(i);
        acc ^= *slot;
    }
    std::hint::black_box(acc);
    assert_eq!(thread_allocations(), before);
}
