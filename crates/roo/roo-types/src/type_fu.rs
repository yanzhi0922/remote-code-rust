//! Type-level utility definitions.
//!
//! Derived from `packages/types/src/type-fu.ts`.
//! In Rust these are expressed as marker traits or type-level constants
//! rather than TypeScript type aliases. The actual runtime utilities
//! are trivial; this module exists to keep the type manifest in sync
//! with the TS source.

/// Placeholder for the TS `Keys<T>` type alias.
///
/// In Rust, use `std::any::type_name` or generic bounds as needed.
/// This struct is a marker only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeFuKeys;

/// Placeholder for the TS `Values<T>` type alias.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeFuValues;

/// Placeholder for the TS `Equals<X, Y>` type alias.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeFuEquals;

/// Placeholder for the TS `AssertEqual<T>` type alias.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeFuAssertEqual;
