//! In-repo `macro_rules!` used only where a helper fn cannot express
//! the deduplication: trait `impl` blocks (the language forbids a
//! blanket `impl<T: Process> Drop for T`) and method definitions
//! (accessors). These replace what the getset derive dependency used
//! to generate, without reintroducing the dependency.
//!
//! Dropping getset removed three lockfile entries (`getset`,
//! `proc-macro-error2`, `proc-macro-error-attr2`) — a proc-macro stack
//! whose original crate, `proc-macro-error`, is flagged unmaintained
//! (RUSTSEC-2024-0370); the `-error2` crates are the fork that advisory
//! forced. `macro_rules!` needs none of it.

/// `pub fn field(&self) -> &Type { &self.field }` for each listed
/// field, with the given doc comment.
macro_rules! ref_getters {
    ($ty:ty { $($(#[$doc:meta])* $field:ident: $ret:ty),+ $(,)? }) => {
        impl $ty {
            $(
                $(#[$doc])*
                pub fn $field(&self) -> &$ret {
                    &self.$field
                }
            )+
        }
    };
}
pub(crate) use ref_getters;

/// `pub fn field(&self) -> Type { self.field }` for each listed `Copy`
/// field, with the given doc comment.
macro_rules! copy_getters {
    ($ty:ty { $($(#[$doc:meta])* $field:ident: $ret:ty),+ $(,)? }) => {
        impl $ty {
            $(
                $(#[$doc])*
                pub fn $field(&self) -> $ret {
                    self.$field
                }
            )+
        }
    };
}
pub(crate) use copy_getters;

/// `impl Drop` delegating to `Process::stop` for each listed process
/// type. Opt-in per type because Rust forbids the blanket
/// `impl<T: Process> Drop for T` this would otherwise be.
macro_rules! impl_stop_on_drop {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Drop for $ty {
                fn drop(&mut self) {
                    self.stop();
                }
            }
        )+
    };
}
pub(crate) use impl_stop_on_drop;
