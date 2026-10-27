//! One bounded pool for simulations and resets; never touches Rayon's global pool.
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::OnceLock;

pub(crate) fn install<R: Send>(work: impl FnOnce() -> R + Send) -> R {
    static POOL: OnceLock<ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(4)
            .thread_name(|i| format!("blood-physics-{i}"))
            .build()
            .expect("create four physics threads")
    })
    .install(work)
}
