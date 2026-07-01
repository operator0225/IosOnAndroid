//! dyld-equivalent symbol resolution.
//!
//! Real dyld binds a loaded image's undefined symbols against whatever
//! dylibs it depends on. We have no Apple dylibs to bind against (see
//! `../../docs/LEGAL.md`), so instead this resolves an image's imports
//! against a registry of our own shim implementations — `libSystem`
//! replacements, eventually Foundation/UIKit stand-ins. Binding itself
//! (writing resolved addresses into the image's `__DATA` pointer slots)
//! is a later step once there's a live mapped process to write into; this
//! module only answers "can every import be satisfied, and with what".

use std::collections::HashMap;

use loader::Import;

/// Maps symbol names to the address of this project's own implementation.
#[derive(Debug, Default)]
pub struct Registry {
    symbols: HashMap<String, u64>,
}

impl Registry {
    pub fn new() -> Self {
        Registry::default()
    }

    pub fn define(&mut self, name: impl Into<String>, addr: u64) -> &mut Self {
        self.symbols.insert(name.into(), addr);
        self
    }

    pub fn is_defined(&self, name: &str) -> bool {
        self.symbols.contains_key(name)
    }

    /// Attempts to resolve every import against this registry. Missing
    /// symbols are reported, not silently ignored or guessed at — a
    /// partially-resolved image should not be run.
    pub fn resolve(&self, imports: &[Import]) -> ResolveReport {
        let mut resolved = Vec::with_capacity(imports.len());
        let mut missing = Vec::new();
        for import in imports {
            match self.symbols.get(&import.name) {
                Some(&addr) => resolved.push((import.name.clone(), addr)),
                None => missing.push(import.name.clone()),
            }
        }
        ResolveReport { resolved, missing }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveReport {
    pub resolved: Vec<(String, u64)>,
    pub missing: Vec<String>,
}

impl ResolveReport {
    pub fn is_fully_resolved(&self) -> bool {
        self.missing.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_symbols() {
        let mut reg = Registry::new();
        reg.define("_exit", 0x1000).define("_write", 0x2000);

        let imports = vec![
            Import {
                name: "_write".to_string(),
            },
            Import {
                name: "_exit".to_string(),
            },
        ];
        let report = reg.resolve(&imports);

        assert!(report.is_fully_resolved());
        assert_eq!(
            report.resolved,
            vec![
                ("_write".to_string(), 0x2000),
                ("_exit".to_string(), 0x1000),
            ]
        );
    }

    #[test]
    fn reports_missing_symbols_without_guessing() {
        let mut reg = Registry::new();
        reg.define("_exit", 0x1000);

        let imports = vec![
            Import {
                name: "_exit".to_string(),
            },
            Import {
                name: "_NSLog".to_string(), // not implemented yet
            },
        ];
        let report = reg.resolve(&imports);

        assert!(!report.is_fully_resolved());
        assert_eq!(report.resolved, vec![("_exit".to_string(), 0x1000)]);
        assert_eq!(report.missing, vec!["_NSLog".to_string()]);
    }

    #[test]
    fn empty_import_list_is_trivially_resolved() {
        let reg = Registry::new();
        assert!(reg.resolve(&[]).is_fully_resolved());
    }

    #[test]
    fn is_defined_reflects_registrations() {
        let mut reg = Registry::new();
        assert!(!reg.is_defined("_exit"));
        reg.define("_exit", 0x1000);
        assert!(reg.is_defined("_exit"));
    }
}
