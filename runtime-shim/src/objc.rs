//! A clean-room Objective-C object/class/dispatch model.
//!
//! This covers the *data model and lookup logic* of `objc_msgSend` — class
//! hierarchies, method tables, ivar layout, and (crucially, and easy to
//! get wrong) Objective-C's "sending a message to `nil` is legal and
//! returns nil/zero" semantics. It deliberately does not implement the
//! real `objc_msgSend`'s ARM64 calling-convention trampoline (receiver in
//! `x0`, selector in `x1`, args in `x2..`, tail-calling into the resolved
//! `Imp`): that needs real ARM64 execution to verify and belongs with
//! `syscall-shim::ptrace`'s honesty caveat, not here. Nor does it parse
//! real classes out of a Mach-O's `__objc_classlist`/`__objc_methname`
//! sections yet — that's real, well-documented-but-intricate work for a
//! later pass; see `../../docs/ROADMAP.md` Phase 3.
//!
//! None of this is derived from Apple's `objc4` runtime source — see
//! `../../docs/LEGAL.md`. The data model (isa-less here; we use `Arc`
//! instead of raw `isa` pointers) and the walk-up-the-superclass-chain
//! lookup algorithm are the same *idea* every Objective-C runtime
//! implements, not copied code.

use std::collections::HashMap;
use std::sync::Arc;

/// A selector — a method name like `init` or `setValue:forKey:`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sel(String);

impl Sel {
    pub fn new(name: impl Into<String>) -> Self {
        Sel(name.into())
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Sel {
    fn from(s: &str) -> Self {
        Sel::new(s)
    }
}

/// A resolved method implementation. Opaque here on purpose: what this
/// address points to (a real ARM64 function vs. a Rust shim) is a
/// decision for whoever populates the method table, not this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imp(pub u64);

#[derive(Debug, Clone)]
pub struct IvarDecl {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

pub struct Class {
    pub name: String,
    pub superclass: Option<Arc<Class>>,
    instance_size: usize,
    methods: HashMap<Sel, Imp>,
    ivars: Vec<IvarDecl>,
}

impl Class {
    /// A root class (like `NSObject`) with no superclass.
    pub fn root(name: impl Into<String>) -> Self {
        Class {
            name: name.into(),
            superclass: None,
            instance_size: 0,
            methods: HashMap::new(),
            ivars: Vec::new(),
        }
    }

    pub fn derive(name: impl Into<String>, superclass: Arc<Class>) -> Self {
        let instance_size = superclass.instance_size;
        Class {
            name: name.into(),
            superclass: Some(superclass),
            instance_size,
            methods: HashMap::new(),
            ivars: Vec::new(),
        }
    }

    pub fn add_method(&mut self, sel: impl Into<Sel>, imp: Imp) -> &mut Self {
        self.methods.insert(sel.into(), imp);
        self
    }

    /// Appends an ivar after whatever this class (or its superclasses)
    /// already declared, growing `instance_size` to match — a simplified
    /// stand-in for the real ABI's per-type alignment rules.
    pub fn add_ivar(&mut self, name: impl Into<String>, size: usize) -> &mut Self {
        let offset = self.instance_size;
        self.ivars.push(IvarDecl {
            name: name.into(),
            offset,
            size,
        });
        self.instance_size += size;
        self
    }

    pub fn instance_size(&self) -> usize {
        self.instance_size
    }

    /// All ivars declared directly on this class (not inherited ones).
    pub fn own_ivars(&self) -> &[IvarDecl] {
        &self.ivars
    }

    /// Whether `self` is `other` or a (possibly indirect) subclass of it —
    /// what `-isKindOfClass:` is built on.
    pub fn is_kind_of(&self, other: &str) -> bool {
        if self.name == other {
            return true;
        }
        match &self.superclass {
            Some(sup) => sup.is_kind_of(other),
            None => false,
        }
    }

    /// The core of `objc_msgSend`'s dispatch logic: look for `sel` on this
    /// class, then walk up the superclass chain.
    pub fn resolve(&self, sel: &Sel) -> Option<Imp> {
        if let Some(imp) = self.methods.get(sel) {
            return Some(*imp);
        }
        self.superclass.as_ref().and_then(|s| s.resolve(sel))
    }
}

pub struct Object {
    pub class: Arc<Class>,
    pub ivars: Vec<u8>,
}

impl Object {
    pub fn new(class: Arc<Class>) -> Self {
        let size = class.instance_size();
        Object {
            class,
            ivars: vec![0u8; size],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoesNotRespond {
    pub class: String,
    pub sel: Sel,
}

/// The `objc_msgSend` dispatch decision: given a receiver (or `None` for
/// a `nil` receiver) and a selector, what to call.
///
/// Objective-C's nil-messaging rule matters here: `[nil someMethod]` is
/// legal and evaluates to nil/0/false, *not* a crash — a naive port from
/// a language without that convention would get this wrong. So a `None`
/// receiver short-circuits to `Ok(None)` without even attempting a method
/// lookup.
pub fn objc_msg_send(receiver: Option<&Object>, sel: &Sel) -> Result<Option<Imp>, DoesNotRespond> {
    let Some(receiver) = receiver else {
        return Ok(None);
    };
    receiver
        .class
        .resolve(sel)
        .map(Some)
        .ok_or_else(|| DoesNotRespond {
            class: receiver.class.name.clone(),
            sel: sel.clone(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_method_defined_directly_on_the_class() {
        let mut nsobject = Class::root("NSObject");
        nsobject.add_method("init", Imp(0x1000));
        let nsobject = Arc::new(nsobject);
        let obj = Object::new(nsobject);

        let imp = objc_msg_send(Some(&obj), &Sel::new("init")).unwrap();
        assert_eq!(imp, Some(Imp(0x1000)));
    }

    #[test]
    fn inherits_method_from_superclass() {
        let mut nsobject = Class::root("NSObject");
        nsobject.add_method("description", Imp(0x2000));
        let nsobject = Arc::new(nsobject);

        let subclass = Arc::new(Class::derive("MyThing", Arc::clone(&nsobject)));
        let obj = Object::new(subclass);

        let imp = objc_msg_send(Some(&obj), &Sel::new("description")).unwrap();
        assert_eq!(imp, Some(Imp(0x2000)));
    }

    #[test]
    fn subclass_override_shadows_superclass_method() {
        let mut nsobject = Class::root("NSObject");
        nsobject.add_method("description", Imp(0x2000));
        let nsobject = Arc::new(nsobject);

        let mut subclass = Class::derive("MyThing", Arc::clone(&nsobject));
        subclass.add_method("description", Imp(0x3000));
        let obj = Object::new(Arc::new(subclass));

        let imp = objc_msg_send(Some(&obj), &Sel::new("description")).unwrap();
        assert_eq!(imp, Some(Imp(0x3000)));
    }

    #[test]
    fn unimplemented_selector_reports_does_not_respond() {
        let nsobject = Arc::new(Class::root("NSObject"));
        let obj = Object::new(nsobject);

        let err = objc_msg_send(Some(&obj), &Sel::new("frobnicate")).unwrap_err();
        assert_eq!(
            err,
            DoesNotRespond {
                class: "NSObject".to_string(),
                sel: Sel::new("frobnicate"),
            }
        );
    }

    #[test]
    fn messaging_nil_returns_none_without_lookup() {
        // The whole point: this must NOT panic/deref, and must NOT be
        // treated as "does not respond" — nil messaging is a first-class,
        // legal no-op in Objective-C.
        let result = objc_msg_send(None, &Sel::new("anything"));
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn ivar_offsets_accumulate_across_inheritance() {
        let mut base = Class::root("Base");
        base.add_ivar("baseField", 8);
        let base = Arc::new(base);
        assert_eq!(base.instance_size(), 8);

        let mut derived = Class::derive("Derived", Arc::clone(&base));
        derived.add_ivar("derivedField", 4);

        assert_eq!(derived.instance_size(), 12);
        assert_eq!(derived.own_ivars()[0].name, "derivedField");
        assert_eq!(derived.own_ivars()[0].offset, 8);

        let obj = Object::new(Arc::new(derived));
        assert_eq!(obj.ivars.len(), 12);
    }

    #[test]
    fn is_kind_of_walks_the_hierarchy() {
        let nsobject = Arc::new(Class::root("NSObject"));
        let middle = Arc::new(Class::derive("Middle", Arc::clone(&nsobject)));
        let leaf = Class::derive("Leaf", Arc::clone(&middle));

        assert!(leaf.is_kind_of("Leaf"));
        assert!(leaf.is_kind_of("Middle"));
        assert!(leaf.is_kind_of("NSObject"));
        assert!(!leaf.is_kind_of("SomethingElse"));
    }
}
