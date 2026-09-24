//! `threads=` on the gate's examples: a pinned thread count reaches every default the
//! tick reads, and clearing it gives the machine's default back. Its own test binary,
//! because the override is process-global.

#[test]
fn a_pinned_thread_count_is_the_default_until_cleared() {
    let machine = cubarium_voxel::default_threads();
    cubarium_voxel::set_thread_override(3);
    assert_eq!(cubarium_voxel::thread_override(), Some(3));
    assert_eq!(cubarium_voxel::default_threads(), 3);
    cubarium_voxel::set_thread_override(0);
    assert_eq!(cubarium_voxel::default_threads(), machine);
}
