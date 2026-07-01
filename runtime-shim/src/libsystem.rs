//! A minimal `libSystem` subset: `extern "C"` trampolines matching what a
//! Darwin binary expects to link against (`_malloc`, `_free`, ...),
//! forwarding to bionic's real implementations underneath. This is the
//! "libSystem subset over bionic" piece of `../../docs/ROADMAP.md` Phase
//! 3 — deliberately tiny (four functions) rather than a sweeping,
//! untested claim of libSystem coverage. Real Darwin symbol names are
//! prefixed with an underscore (the historical C symbol-mangling
//! convention Mach-O still uses), which is why the registry keys below
//! look like `_malloc` rather than `malloc`.

use std::os::raw::{c_char, c_void};

use crate::registry::Registry;

/// # Safety
/// Same contract as `libc::malloc`.
pub unsafe extern "C" fn shim_malloc(size: usize) -> *mut c_void {
    libc::malloc(size)
}

/// # Safety
/// Same contract as `libc::free`.
pub unsafe extern "C" fn shim_free(ptr: *mut c_void) {
    libc::free(ptr)
}

/// # Safety
/// Same contract as `libc::memcpy`.
pub unsafe extern "C" fn shim_memcpy(
    dst: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    libc::memcpy(dst, src, n)
}

/// # Safety
/// Same contract as `libc::strlen`.
pub unsafe extern "C" fn shim_strlen(s: *const c_char) -> usize {
    libc::strlen(s)
}

/// Registers this module's shims into `registry` under their Darwin
/// symbol names, so `runtime_shim::Registry::resolve` can satisfy a
/// binary's imports of them.
pub fn register(registry: &mut Registry) {
    registry.define("_malloc", shim_malloc as *const () as u64);
    registry.define("_free", shim_free as *const () as u64);
    registry.define("_memcpy", shim_memcpy as *const () as u64);
    registry.define("_strlen", shim_strlen as *const () as u64);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn register_defines_expected_symbols() {
        let mut reg = Registry::new();
        register(&mut reg);
        for name in ["_malloc", "_free", "_memcpy", "_strlen"] {
            assert!(reg.is_defined(name), "{name} should be registered");
        }
        assert!(!reg.is_defined("_printf")); // not in this minimal subset yet
    }

    #[test]
    fn registered_malloc_free_round_trip_through_real_addresses() {
        // Proves the registered u64 is a real, callable function pointer
        // to the shim (and thus to bionic's malloc/free underneath), not
        // just a placeholder value.
        let mut reg = Registry::new();
        register(&mut reg);
        let report = reg.resolve(&[loader::Import {
            name: "_malloc".to_string(),
        }]);
        let (_, addr) = report.resolved[0];

        type MallocFn = unsafe extern "C" fn(usize) -> *mut c_void;
        let malloc_fn: MallocFn = unsafe { std::mem::transmute(addr as usize) };

        let ptr = unsafe { malloc_fn(64) };
        assert!(!ptr.is_null());
        unsafe {
            shim_free(ptr);
        }
    }

    #[test]
    fn shim_memcpy_matches_source_bytes() {
        let src = *b"hello, iOS!";
        let mut dst = [0u8; 11];
        unsafe {
            shim_memcpy(
                dst.as_mut_ptr() as *mut c_void,
                src.as_ptr() as *const c_void,
                src.len(),
            );
        }
        assert_eq!(&dst, &src);
    }

    #[test]
    fn shim_strlen_matches_rust_str_len() {
        let s = CString::new("Darwin binary compatibility layer").unwrap();
        let len = unsafe { shim_strlen(s.as_ptr()) };
        assert_eq!(len, "Darwin binary compatibility layer".len());
    }
}
