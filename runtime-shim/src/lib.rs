//! dyld-compatible symbol resolution, a clean-room Objective-C
//! object/dispatch model, ARC refcounting, and a minimal `libSystem`
//! subset, all backed by real Linux/Android primitives underneath.
//!
//! See `../docs/ARCHITECTURE.md` §3 for how this fits the overall stack,
//! and `../docs/ROADMAP.md` Phase 2/3 for exactly what's implemented vs.
//! still open (real ARM64 `objc_msgSend` invocation, parsing classes out
//! of a Mach-O's `__objc_classlist`, Mach IPC/ports, real Darwin thread
//! creation).

pub mod arc;
pub mod libsystem;
pub mod objc;
pub mod registry;
pub mod sync;

pub use arc::{RefCounted, ReleaseOutcome};
pub use objc::{objc_msg_send, Class, DoesNotRespond, Imp, Object, Sel};
pub use registry::{Registry, ResolveReport};
pub use sync::FutexMutex;
