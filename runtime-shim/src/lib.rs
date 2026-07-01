//! dyld-compatible symbol resolution, and Darwin threading-primitive
//! shims backed by real Linux/Android primitives underneath.
//!
//! See `../docs/ARCHITECTURE.md` §3 for how this fits the overall stack.
//! The Objective-C runtime and `libSystem` C-library subset described
//! there are not implemented yet — this crate currently covers only the
//! two pieces of Phase 2 that are self-contained enough to build and test
//! honestly right now: symbol resolution (`registry`) and a futex mutex
//! (`sync`). See `../docs/ROADMAP.md` Phase 2 for what's still open
//! (Mach IPC/ports, the full `bsdthread_create`-style thread-creation
//! path, ARC).

pub mod registry;
pub mod sync;

pub use registry::{Registry, ResolveReport};
pub use sync::FutexMutex;
