# Rust Style Guide

This guide defines a consistent style for handwritten Rust code. It applies to libraries, applications, tools, games, graphical software, and systems components. Its emphasis is the code a reader sees: names, signatures, types, expressions, ownership, errors, and module boundaries.

Use direct, conventional Rust. Make the meaning of a value, the ownership of data, and the possible outcomes of an operation apparent at the point of use. Add an abstraction when it expresses a real concept or removes meaningful duplication.

Follow the [official Rust Style Guide][style] wherever it conflicts with a rule here. Use the [Rust API Guidelines][api] for supporting API conventions. The requirements in this document are stated directly; external references explain language behavior and provide further detail.

## How to apply this guide

An imperative such as "use," "keep," or "do not" is a requirement. "Prefer" describes the default; depart from it for a concrete reason. "May" identifies an allowed choice. Keep exceptions narrow and explain a non-obvious exception next to the relevant code. Correctness and soundness remain requirements in every case.

API design requirements are binding for libraries intended for other projects. For code confined to one application, they are strong recommendations: favor direct solutions to actual needs over speculative reuse. Formatting, safety, and implementation restrictions still apply. Favor code that readers can understand and maintain, conventions that tools can process consistently, and familiar Rust idioms. Apply rules consistently so reviews can focus on behavior.

Apply a rule when the corresponding feature is used. Async Rust, Tokio, `std`, and any particular external crate are optional. Generated or vendored code may keep its own conventions. Examples illustrate local coding choices; snippets labeled as excerpts assume the surrounding declarations shown or described nearby.

## Contents

- [Formatting](#formatting)
- [Naming](#naming)
- [Imports](#imports)
- [Modules and files](#modules-and-files)
- [Functions](#functions)
- [Structs and enums](#structs-and-enums)
- [Traits and conversions](#traits-and-conversions)
- [Ownership and borrowing](#ownership-and-borrowing)
- [Control flow and expressions](#control-flow-and-expressions)
- [Iteration and collections](#iteration-and-collections)
- [Errors with thiserror and anyhow](#errors-with-thiserror-and-anyhow)
- [Panics and assertions](#panics-and-assertions)
- [Concurrency](#concurrency)
- [Async code and Tokio](#async-code-and-tokio)
- [Text paths and numbers](#text-paths-and-numbers)
- [Documentation and comments](#documentation-and-comments)
- [Tests](#tests)
- [Attributes macros and unsafe code](#attributes-macros-and-unsafe-code)
- [Language and API boundaries](#language-and-api-boundaries)
- [References](#references)

## Formatting

### Let rustfmt determine layout

Format code with the repository's approved formatter, normally `cargo fmt`, and use the shared project configuration. Use four spaces rather than tabs and a 100-character target width. Let the formatter determine line breaks, trailing commas, and indentation. Do not manually align unrelated statements or maintain a competing layout.

Separate items and statements with zero or one blank line. Use one between logical steps and keep related statements together. Remove trailing whitespace, including on blank and comment lines; preserve literal values by using escapes where necessary. Do not use decorative comment banners or large blocks of empty lines to organize a file; use modules and ordinary headings in documentation.

Use `#[rustfmt::skip]` only for meaningful manual alignment, such as a data table or matrix, or for generated code that needs to preserve its generated layout. Prefer a reproducible generator for the latter. Explain a non-obvious reason. Personal preference is not sufficient.

Use the [official formatting conventions][style]. Prefer block indentation to alignment under an opening delimiter. Put a trailing comma after the last entry in a multiline comma-separated list, except where the syntax forbids it, such as after a struct update expression. Let `rustfmt` choose the compact form for small constructs.

Configure `max_width = 100`. When the pinned formatter supports them, also set `hex_literal_case = "Lower"`, `format_macro_matchers = true`, and `imports_granularity = "Crate"`. Format macro matchers as ordinary Rust syntax where possible. Do not require a nightly toolchain solely for a formatter option. Follow the source-level rules even when the formatter cannot enforce them. [rustfmt configuration][rustfmt-config]

### Make literals readable

Separate long numeric literals into meaningful groups, such as `1_000_000` or `0xfeed_beef`. Use lowercase hexadecimal digits consistently. Name repeated literals when they represent a domain rule, unit, sentinel, or limit. Keep an obvious local value inline when a constant would add no meaning.

Use escapes for invisible or ambiguous characters. Use a raw string when it makes a literal with quotes or backslashes easier to read. Choose the smallest number of raw-string delimiters that works.

## Naming

### Use conventional Rust names

| Item                                  | Convention             | Example                                 |
| ------------------------------------- | ---------------------- | --------------------------------------- |
| Modules and source files              | `snake_case`           | `record_id.rs`                          |
| Functions, methods, fields, variables | `snake_case`           | `read_header`, `byte_len`               |
| Types, traits, enum variants          | `UpperCamelCase`       | `RecordId`, `DecodeError`, `Ready`      |
| Constants and statics                 | `SCREAMING_SNAKE_CASE` | `MAX_NAME_BYTES`                        |
| Declarative macros                    | `snake_case`           | `define_flags!`                         |
| Boolean queries                       | A predicate            | `is_empty`, `has_children`, `can_retry` |

Treat acronyms as words in type names: `HttpClient`, `Utf8Error`, and `Uuid`, not `HTTPClient` or `UUID`. Preserve established spellings in external interfaces when necessary. If a name is a reserved word, use a raw identifier such as `r#type` or a trailing underscore such as `type_`; do not misspell it. See the [API naming conventions][naming].

### Name the concept and the unit

Choose names that remain meaningful outside the statement where they are declared. Prefer `record_count`, `byte_offset`, and `timeout` to `n`, `pos`, and `value` when those distinctions matter.

Short names are appropriate for a small, obvious scope: an index in a simple loop, coordinates in a formula, or `T` in a generic container. Do not shorten words into unfamiliar abbreviations or abbreviate a word by dropping letters from its middle. Established names such as `len`, `id`, `io`, and `utf8` are fine.

Do not use vague suffixes such as `Manager`, `Helper`, `Processor`, or `Data` as a substitute for identifying a responsibility. Avoid names such as `new_value2` that describe editing history. Do not encode the storage type into a name unless it distinguishes relevant values.

Put units in the type where practical. Otherwise include them in the name: `elapsed_ms`, `size_bytes`, or `angle_radians`. Do not make a reader infer whether a number counts elements, bytes, or seconds.

### Name methods for their behavior

Use `name()` for a property accessor, not `get_name()`. Keep `get` for operations with established lookup semantics, such as a possibly missing indexed element.

Use related method names consistently:

| Behavior                                                      | Typical name                       |
| ------------------------------------------------------------- | ---------------------------------- |
| Borrow a property, or return a small `Copy` property by value | `name()`, `items()`                |
| Mutably borrow a property                                     | `name_mut()`, `items_mut()`        |
| Replace a property with an owned value                        | `set_name(value)`                  |
| Replace and return the previous value                         | `replace_name(value)`              |
| Return the old value and leave `None` or a valid default      | `take_name()`                      |
| Access a property through a closure                           | `with_name(f)`, `with_mut_name(f)` |
| Borrowing conversion or view                                  | `as_bytes()`, `as_str()`           |
| Produce a converted value while retaining the original        | `to_owned()`, `to_string()`        |
| Consume the value in a conversion                             | `into_bytes()`, `into_inner()`     |

Use `as_` for a cheap borrowed view, normally taking `&self` or `&mut self`. Use `to_` for a conversion that retains the original, normally taking `&self`; it may allocate or perform substantial work. Use `into_` for a conversion that consumes `self`. A small `Copy` receiver may be taken by value where that is conventional. Use `try_` to distinguish a fallible alternative when there is a meaningful pair. A naturally fallible operation such as `parse` or `open` does not need a redundant `try_` prefix. [Conversion naming][naming]

Use `new` for the primary constructor and `new_with_<detail>` for an additional construction path when that pattern fits. A constructor returning `Result` must use `try_new` or an appropriately qualified `try_new_with_<detail>` name. For a value-to-value conversion, use `from_<source>` with the corresponding `From` implementation when that trait's contract applies; use a fallible conversion for validation. Do not rename required standard trait methods or externally imposed APIs.

For collection-like APIs, provide `is_empty` alongside `len`. Name ordinary borrowed, mutable, and consuming iteration methods `iter`, `iter_mut`, and `into_iter` when they have those meanings.

### Name generic parameters and lifetimes proportionately

Prefer short, indicative single-letter generic parameter names, following the official Style Guide: `R: Read`, `E`, `K`, and `V`. Use `T` or `U` for broadly unconstrained types. A short descriptive name is acceptable when it materially clarifies distinct roles; do not make longer names the default. Associated type names such as `Item` and `Output` describe their roles. [Official generics style][style-items]

Elide lifetimes when Rust's rules express the intended relationship, and use `'_` where an anonymous lifetime is sufficient. Give named lifetimes descriptive names, especially on types: `'input`, `'arena`, or `'request`. Single letters are appropriate for very generic types such as `Cow`, for up to two simple local input/output relationships, and for local higher-ranked bounds such as `for<'a>`. Use descriptive names when more local lifetimes are needed. Do not add lifetime parameters merely to make a signature look more complete.

## Imports

### Keep imports explicit and grouped

Group module-level imports in this order, with a blank line between groups:

1. `std`, `core`, and `alloc`.
2. External crates.
3. Items from the current crate.

Within each group, use version sorting, so `item8` precedes `item16`; put `self` and `super` first where applicable. Combine imports from the same crate into one `use` statement with nested brace groups for submodules. Apply this to standard-library, external-crate, and current-crate imports. Keep imports separate when their visibility or conditional attributes differ. Let the formatter handle ordering and line breaks, including multiline nested groups. Configure automatic merging with `imports_granularity = "Crate"` when supported rather than assuming the formatter does it by default. [Import formatting][style-items]

```rust
use std::{collections::{BTreeMap, BTreeSet}, io, path::{Path, PathBuf}};

use tokio::task::JoinHandle;

use crate::record::{Record, RecordId};
```

The last import is a module-layout excerpt: it assumes the project defines `crate::record`.

Do not use wildcard imports except for these cases:

- `use super::*` in inline test modules.
- A local import of enum variants inside a method on that enum or a function primarily concerned with it, when it makes patterns clearer.
- An established third-party prelude whose use is justified during review.
- A private adapter module that combines generated bindings for the same C++ namespace from several crates. The adapter may glob re-export those bindings internally, but the adapter module itself must remain private.

Keep other public re-exports explicit, including those in non-leaf modules. Avoid defining new preludes. Prefer qualified enum expressions such as `State::Ready`, except for prelude variants such as `Some` and `Ok`. A local variant import for patterns does not change that expression convention. [Enum expression formatting][style-expressions]

### Import types and qualify free functions

Import ordinary type and trait names and use them without module qualification. Call free functions from other modules through the imported module name, so an unqualified function normally belongs to the current module.

```rust
use std::{fs, io, path::Path};

fn read_bytes(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}
```

Keep commonly ambiguous types qualified, such as `io::Error`, or give them a specific alias such as `IoError`. Qualification is also appropriate for a large related family of types, macros or generated code where imports are impractical, or a dependency-heavy module where the imports would add little clarity. Do not rename a type just to save a few characters.

Directly imported free functions are exceptions when their names already disambiguate them, as can happen in C++ bindings, or when the module name itself collides across crates. Keep exceptions consistent within the module.

### Scope trait imports to their use

A trait needed only for method resolution in one function should normally be imported there. Use `as _` when the name itself is unnecessary. Keep the import at module scope when many functions use it or signatures name the trait.

```rust
fn parse_setting(input: &str) -> anyhow::Result<usize> {
    use anyhow::Context as _;

    input.parse().context("parsing the item count")
}
```

Use `crate::` for clear paths to other parts of the current crate. Use `super::` for a genuinely local parent relationship. Avoid long chains of `super::super::...`; they make a module's position part of every import.

## Modules and files

### Organize modules around concepts

A module should own a coherent concept: a parser, an identifier, a geometry operation, or a collection of closely related types. Name it for that concept. If a leaf module has a primary user-defined item such as `RecordId`, name the module and source file after it: `record_id` and `record_id.rs`. Choose a descriptive name freely when no single item is primary.

Keep tightly related types together. Do not require one file per type. Conversely, do not let `utils`, `common`, or `helpers` become collections of unrelated code. Put a helper beside its callers until it has a clear independent responsibility.

Choose files to make navigation straightforward. A useful conventional arrangement is:

| Path                                     | Responsibility                                                        |
| ---------------------------------------- | --------------------------------------------------------------------- |
| `src/lib.rs`                             | Crate documentation, public surface, and small root-level definitions |
| `src/record.rs`                          | The main record abstraction or its module facade                      |
| `src/record/decode.rs`                   | Decoding implementation owned by the record module                    |
| `src/record/encode.rs`                   | Encoding implementation owned by the record module                    |
| `src/main.rs`, when present              | Executable initialization and coordination                            |
| `tests/record_roundtrip.rs`, when useful | A test of the crate's public interface                                |

This is an illustration of relationships, not a required list of directories. Use names from the actual domain. A small crate may keep its implementation in `lib.rs`.

Prefer `record.rs` plus `record/child.rs` for new module trees. Existing, consistent `record/mod.rs` layouts are acceptable. Do not introduce `#[path]` merely to produce an unusual file hierarchy. [Rust modules][modules]

### Separate a module's public surface from its implementation

Keep implementation modules private when callers only need their exported types and functions. Re-export the deliberate public API explicitly.

The following is a parent-module excerpt:

```rust
pub use decode::{DecodeError, Decoder};
pub use encode::Encoder;

mod decode;
mod encode;
```

Keep non-leaf modules focused on out-of-line module declarations and re-exports. Put API implementations in leaf modules. A small amount of core code that ties children together is an allowed exception, but a substantial implementation belongs in a leaf. Inline test modules and cross-language binding modules such as `#[cxx::bridge]` do not stop an implementation module from being a leaf for this rule.

### Use the narrowest useful visibility

Start with private items. Use `pub(crate)` for a crate-internal interface and `pub` for the intended external API. Do not use `pub(in ...)`, `pub(super)`, or `pub(self)`. When several modules need a private implementation detail, place it in a suitable shared module or expose a deliberate `pub(crate)` interface. Do not make an item public merely because a test or a distant module wants convenient access.

Keep fields private when construction or mutation must preserve an invariant. Public fields are appropriate for a deliberately transparent record with no hidden validity rules. A public method's signature must not accidentally expose a private implementation type or a dependency that callers should not need to know. [Visibility and privacy][visibility]

### Keep a predictable order within a file

Put module documentation and inner attributes at the beginning. If `extern crate` declarations are actually needed, place them before other items and sort them alphabetically. Put `use` statements, including re-exports, before out-of-line `mod name;` declarations; put both before other items. Version-sort imports within each group and module declarations, with `self` and `super` first where applicable. Preserve any ordering required by macros; do not automatically move `#[macro_use]` modules. Keep a type near its inherent implementation, related trait implementations nearby, and inline tests at the end. [Official item order][style-items]

Within an inherent implementation, group constructors, queries, transformations, and mutation operations in a consistent order. Keep private helpers near the operations they explain. Prefer conceptual order over alphabetical order, and avoid scattering one type's methods across many files without a clear reason.

## Functions

### Give each function one coherent job

A function should express one operation at its own level of abstraction. Extract a helper when it names a meaningful step, isolates a separate rule, or removes substantial duplication. Do not extract a one-line expression merely to make a function shorter.

Keep state changes and early exits visible. A reader should not need to trace several layers of callbacks to discover that a function mutates an argument, consumes a value, or reports failure.

Choose an inherent method when the operation belongs to a type and its invariants. Use a free function when the operation has no natural owning type. Do not introduce an empty struct solely as a namespace for functions.

### Accept the data the operation needs

Use the narrowest parameter type that expresses the contract and serves the actual callers.

| Need                     | Prefer       | Avoid unless the wrapper matters |
| ------------------------ | ------------ | -------------------------------- |
| Read text                | `&str`       | `&String`                        |
| Read a sequence          | `&[T]`       | `&Vec<T>`                        |
| Modify existing elements | `&mut [T]`   | `&mut Vec<T>`                    |
| Read a filesystem path   | `&Path`      | `&PathBuf`                       |
| Borrow an inner value    | `&T`         | `&Box<T>`, `&Arc<T>`             |
| Read an optional value   | `Option<&T>` | `&Option<T>`                     |

Use the container itself when the operation needs its capabilities. For example, resizing requires `&mut Vec<T>`, replacing an optional value can require `&mut Option<T>`, and duplicating a shared handle can require access to `Arc<T>`. Noncontiguous text such as a rope or cord may require its own borrowed interface rather than `&str`.

Pass small `Copy` values by value. Take an owned `String`, `Vec<T>`, or other value when the function retains, transfers, or consumes it. Do not accept a borrow and then clone it unconditionally when ownership is the real contract.

```rust
fn append_suffix(output: &mut String, suffix: Option<&str>) {
    if let Some(suffix) = suffix {
        output.push_str(suffix);
    }
}
```

Callers can adapt owned optional data with `as_ref` or `as_deref`. These adapters preserve the original ownership. [Option adapters][option]

### Use generics for a useful range of inputs

Use concrete parameters by default in application and implementation code. Application code should almost never accept `impl Into<T>` merely to save a conversion at the call site. In a reusable library, consider `impl Into<T>` when callers commonly hold several convertible types and call-site convenience justifies the more complex signature, inference, and generated code. Do not add conversion generics routinely.

Use `AsRef` for a cheap borrowed view and `Into` for an owned conversion. Convert once near the start of the function when that makes the remainder simpler. Do not add multiple conversion traits to every parameter or pass a generic conversion object through layers that only need the converted value.

For highly reusable library operations that only traverse inputs, accept an iterator or `impl IntoIterator`. Use a slice when the function needs contiguous access or indexing. A slice is also appropriate in application code when it is simpler and iterator flexibility or allocation avoidance is not needed. Use `impl AsRef<Path>` or a similar borrowed conversion when actual callers benefit. See the [API flexibility guidelines][flexibility].

Require only the bounds the operation needs. Put a bound on the method that needs it instead of the entire type when practical. Do not add `Clone`, `Send`, `Sync`, or `'static` as routine defaults. For callbacks, choose `FnOnce`, `FnMut`, or `Fn` according to how the callback is invoked.

### Make outputs express the outcome

Return a value instead of mutating an output argument unless in-place mutation, reuse of an existing destination, or an external interface is part of the operation.

Use `Option<T>` for normal absence or a simple anticipated failure whose cause needs no further explanation, such as checked arithmetic overflow. Use `Result<T, E>` when callers need failure details, and `Result<Option<T>, E>` when an operation can fail or succeed without finding a value. Reserve panics for defects and violated preconditions, subject to the [panic rules](#panics-and-assertions). Do not use an empty string, zero, or an arbitrary integer as an undocumented absence or failure sentinel.

Return a named struct when several output fields have distinct meanings. A small local pair is fine when its interpretation is conventional, such as a key and value or the two parts of a split.

Prefer an iterator return type, concrete or `impl Iterator`, for reusable traversals and transformations. Returning a `Vec<T>` is appropriate when most callers need one, the implementation naturally constructs or already owns one, random indexing is likely, the operation explicitly creates an owned collection, or a direct loop is clearer and its allocation cost is acceptable. Application code may choose a vector for simplicity when iterator flexibility and performance offer no clear benefit.

### Make arguments understandable at call sites

Do not expose a string of positional flags or several indistinguishable primitive arguments. Use an enum for a meaningful choice and an options struct for related settings.

An API declaration and call-site excerpt:

```rust
#[derive(Clone, Copy, Debug)]
enum CaseSensitivity {
    Sensitive,
    Insensitive,
}

// The third argument now explains the choice at the call site.
// find_name(names, query, CaseSensitivity::Insensitive)
```

A boolean is appropriate when it directly represents a boolean property. The restriction concerns opaque calls such as `find_name(names, query, true)`, where the argument's meaning is hidden. When an existing interface requires an unclear boolean argument, consider a short inline argument comment such as `/* case_sensitive */ true`.

Keep required inputs explicit. Use an options struct or builder when the caller benefits from named optional settings; do not introduce a builder for every constructor.

## Structs and enums

### Represent distinct meanings with distinct types

Use a newtype when two values have different meanings, when an invariant deserves enforcement, when a type needs its own API, or when its underlying representation may need to change independently of callers. A type alias creates another name for the same type; it does not prevent values from being mixed.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RecordId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Revision(u64);
```

Use an alias for a useful spelling of an existing type, such as a complicated map type or a module-local result type. Do not use an alias where the purpose is validation or type safety.

### Give meaningful records named fields

Prefer named fields when a value contains several pieces of domain data. Tuple indexing should remain obvious from the local context.

```rust
struct SourceLocation {
    line: usize,
    column: usize,
}
```

A single-field tuple struct is appropriate for a newtype. Tuples remain useful for short-lived, conventional groupings. Do not return a tuple such as `(String, usize, bool)` from a public operation when readers need to remember a field legend.

### Make invalid states difficult to construct

Use an enum for mutually exclusive states. Put data that belongs to one state in that variant. Avoid independent `Option` fields and flags whose combinations require a hidden validity table.

```rust
enum ParseState {
    Header,
    Body { remaining_bytes: usize },
    Complete,
}
```

Validate at construction and expose methods that preserve the invariant. Reuse standard invariant-bearing types such as `NonZeroUsize` when their meaning fits. Do not create a value in an invalid state and require a later `init` call to repair it.

Avoid a setter for a field when arbitrary replacement would violate the type's rules. Expose the actual operation instead. Returning `&mut` to an internal field is also permission to mutate it freely; apply the same reasoning to mutable accessors.

### Use closure accessors for scoped access

Consider `with_property` and `with_mut_property` when a property lives behind interior mutability or changes require follow-up work. A closure can keep a lock or borrow guard inside the abstraction instead of exposing `MutexGuard` or another implementation detail. Accept a suitable closure, often `FnOnce`, and normally return its result. When both read-only and mutable forms exist, use the distinct names; `with_property` may denote mutation if no paired read-only form exists and the contract is clear.

Document the scope of access and any validation or follow-up work. Ensure invariants remain valid if the callback panics; checking only after a successful return is insufficient for an invariant required by safe callers or unsafe internals.

### Make defaults meaningful

Implement `Default` when there is a useful, valid default value. If a zero-argument `new()` produces that same value, keep `new()` and `Default` consistent.

All-public structs whose purpose is a bag of optional settings must implement `Default`, so callers can override selected fields with struct update syntax. Keep required identifiers, external resources, and validated values without a natural default as separate required inputs. Do not invent a dummy or invalid value to make `Default` available.

Derive `Default` only when the field defaults collectively express the intended default. Otherwise implement it explicitly.

### Choose extensibility deliberately

Mark an enum exposed through a public API used by other teams or independently maintained projects `#[non_exhaustive]` by default. Exceptions are a deliberately permanent closed set, including a stable two-state choice, or a commitment to update every affected caller when variants change. A type with only a few controlled uses may omit the attribute when the migration risk is small. Keep internal enums exhaustive when adding a state should trigger a review of their matches.

Apply the same default to public structs with public fields used across those boundaries. Exceptions are fields expected to remain stable, a commitment to migrate affected callers, a small controlled set of uses, or a deliberate API that permits struct update syntax such as `Options { limit: 8, ..Default::default() }`.

An external caller cannot construct a `#[non_exhaustive]` struct with a literal, even using `..Default::default()`. Supply constructors, a builder, or other supported mutation paths when that attribute prevents literal construction. Private fields enforce representation boundaries; `#[non_exhaustive]` reserves room for API evolution. Neither replaces the other. [The non_exhaustive attribute][nonexhaustive]

## Traits and conversions

### Start with inherent methods

Define a trait to support generic bounds, an `impl Trait` or `dyn Trait` interface, or an extension to a foreign type. Use inherent methods for ordinary behavior belonging to one type.

Do not create a trait for every struct. Do not use traits merely to split a large implementation into categories. A trait is a contract for implementors as well as callers; keep it focused and document any relationship among its methods.

In library function parameters, prefer generics or `impl Trait` over `dyn Trait` so callers retain their concrete types. Code-size concerns can justify dynamic dispatch. Use `dyn Trait` when runtime selection, heterogeneous storage, or an explicit erased interface requires it. Application code may use the simpler suitable interface. Check the trait's [dyn compatibility][traits] rather than assuming every trait can be made into a trait object.

Give extension traits an `Ext` suffix. Their purpose is to add methods to a foreign type or to implementors of a foreign trait; an extension trait is not itself a contract for generic bounds or dynamic dispatch. Prefer having the owner of the underlying type own the extension trait as well. A foundational library or framework may justify an exception for valuable methods expected to become widely known. Keep methods unsurprising and unlikely to collide with other APIs. A capability trait implemented for several foreign types, intended for generic inputs, is a different kind of trait.

### Implement standard traits when the semantics fit

Low-level reusable libraries should anticipate plausible users' trait needs. Application code should add implementations when needed. For a type with public fields, consider the standard traits shared by all its fields; do not add a trait with incorrect semantics merely because deriving it is possible.

Use standard contracts instead of inventing similarly named methods: `FromStr` for parsing a value from text, `From` or `TryFrom` for conversions, `Iterator` for iteration, and `Display` for an intended textual presentation.

Prefer `#[derive(...)]` when its behavior is correct. Write a manual implementation when field-by-field behavior is wrong or the derive introduces bounds the implementation does not need. Do not derive a trait just because another generic function requests it.

Review these traits deliberately:

| Trait                | Required semantic decision                                                                                                                                                                                                                |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Debug`              | Developer-facing output; redact fields that must not be exposed                                                                                                                                                                           |
| `Clone`              | Most value types should implement it; external resources may not have meaningful duplication. State whether cloning duplicates data or a shared handle.                                                                                   |
| `Copy`               | Implement when implicit duplication fits and the type can reasonably remain `Copy` as it evolves. Error types should implement it whenever their representation and semantics permit. Do not remove useful error context to achieve this. |
| `Default`            | Implement for a valid default state, including a zero-argument `new() -> Self`; keep the two consistent.                                                                                                                                  |
| `PartialEq` / `Eq`   | `PartialEq` provides `==`; add `Eq` only when full equivalence, including reflexivity, holds. Do not claim a stronger contract just to enable comparisons.                                                                                |
| `PartialOrd` / `Ord` | `PartialOrd` provides comparisons; add `Ord` only when a total order exists.                                                                                                                                                              |
| `Hash`               | Values that compare equal must produce equal hashes. Implement `Hash` and `Eq` for likely map or cache keys when their contracts fit. Add `Ord` and its prerequisites for naturally totally ordered keys.                                 |

Keep equality, ordering, and hashing consistent. Do not use an arbitrary field ordering when it misrepresents the domain. Follow each trait's documented contract and the [interoperability guidelines][interoperability].

### Use From for infallible conversions and TryFrom for validation

Implement `From`, not `Into`, when converting into a type you can implement it for. Use `TryFrom` when conversion can fail. An infallible conversion must preserve the intended meaning; do not hide a validation panic or a surprising loss of information inside `From`.

This complete example gives a validated value one construction rule:

```rust
use std::num::NonZeroUsize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BatchSize(NonZeroUsize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("batch size must be greater than zero")]
struct InvalidBatchSize;

impl TryFrom<usize> for BatchSize {
    type Error = InvalidBatchSize;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        NonZeroUsize::new(value).map(Self).ok_or(InvalidBatchSize)
    }
}

impl BatchSize {
    #[must_use]
    fn get(self) -> usize {
        self.0.get()
    }
}
```

When a `From` implementation is a primary construction path, also provide a discoverable inherent constructor such as `new`, `new_from_<source>`, or `from_<source>`. Delegate one implementation to the other. A `TryFrom` implementation may similarly have a `try_new` or `try_from_<source>` constructor; keep validation in one place. At a conversion call site, use a target type or a named conversion when `.into()` would leave the result unclear. [From][from] [TryFrom][tryfrom]

### Keep borrowing traits and operators unsurprising

Use `AsRef` to expose a cheap reference conversion. Implement `Borrow` only when the borrowed value preserves the equality, ordering, and hashing behavior required by that trait. They are not interchangeable convenience traits. [Borrow][borrow]

Do not use `Deref` or `DerefMut` to simulate inheritance or to expose every operation of an internal field. Use them for a deliberate pointer-like abstraction with a natural dereference target. [Deref][deref]

Overload operators only for their established mathematical or logical meanings. Do not use an operator merely as compact syntax for an unrelated domain action. Keep indexing faithful to indexed access; use a named method when an operation's meaning would be hidden behind `+`, `*`, or `[]`.

## Ownership and borrowing

### Make ownership visible in the interface

Prefer one clear owner with borrows for temporary access. Pass ownership when the recipient must retain or consume a value. Choose `&self`, `&mut self`, or `self` according to whether an operation observes, modifies, or consumes its receiver. Small `Copy` receivers may also be taken by value for an observing operation, as in `BatchSize::get`.

Do not return references tied to an unrelated temporary. Do not extend a lifetime with a leak, a cast, or an unnecessary `'static` requirement to make an ownership error disappear. Adjust the ownership relationship or return an owned value.

Prefer an owned field when the type is meant to stand independently. Use borrowed fields when the type is intentionally a view into another value. Do not give every struct a lifetime parameter simply to avoid owning a string.

### Clone for an intentional duplicate

Use `clone()` when the code needs a duplicate and the type's cloning semantics are appropriate. Do not add clones mechanically until borrow errors disappear. First check whether the operation should borrow, consume the value, or run in a smaller scope.

A clone can be a reasonable way to simplify ownership. Keep that choice visible. Do not describe all clones as deep copies: cloning a shared handle retains the same underlying object. [Clone][clone]

Prefer unique ownership and borrows to `Rc` or `Arc`. Use shared ownership when the alternative would be unreasonably complex to write, understand, or maintain; make that tradeoff clear in review. Do not wrap a value merely because it is passed to several functions. Consider allocation and copying costs when choosing borrows, clones, or shared handles. When cloning a shared pointer, write `Rc::clone(&value)` or `Arc::clone(&value)` so the kind of duplication is apparent.

### Keep mutation and borrows local

Use immutable bindings by default. Add `mut` where modification occurs, and keep the mutable phase as small as the operation allows.

Use a short block or helper to limit a borrow when it makes the next step clearer. Prefer ordinary borrowing before reaching for interior mutability. If interior mutability is part of a type's design, make the reason and relevant borrowing rules clear in that type.

Use shadowing for a direct refinement, such as raw text becoming parsed data, when retaining one conceptual name helps. Use different names when both values remain relevant or represent different units or meanings.

### Use take and replace for intentional moves

Use `Option::take`, `mem::take`, or `mem::replace` when moving a value out of a field while leaving a valid replacement. Do not clone a value solely to work around a move when the operation is meant to consume the original.

```rust
#[derive(Default)]
struct PendingLabel {
    label: Option<String>,
}

impl PendingLabel {
    fn take_label(&mut self) -> Option<String> {
        self.label.take()
    }
}
```

The replacement must preserve the containing type's invariant. Use `mem::take` only when the default value is valid in that position. [Option::take][option] [mem::take][take]

## Control flow and expressions

### Bind values with patterns

Use `if let` when one variant matters, `let ... else` for a guard that exits early, and `match` when several outcomes require distinct behavior. Do not check `is_some()` or `is_ok()` and then unwrap the same value.

Prefer:

```rust
#[must_use]
fn first_name_if_nonempty(names: &[String]) -> Option<&str> {
    let name = names.first()?;
    let name = name.trim();

    if name.is_empty() {
        return None;
    }

    Some(name)
}
```

Use a pattern when the fallback needs an explicit action:

```rust
fn append_first(output: &mut String, values: &[String]) {
    let Some(first) = values.first() else {
        return;
    };

    output.push_str(first);
}
```

These forms keep the existence of a value connected to the scope where it can be used. Prefer them to a separate check whose conclusion a later operation merely assumes.

Destructure a struct or tuple when it gives used fields clear names. Use `..` for fields intentionally irrelevant to the operation. Do not bind a long list of unused fields.

### Make match coverage deliberate

Match a closed enum exhaustively when every state requires a decision. Avoid `_` when a future variant should force a review of the code.

Use a wildcard when the fallback genuinely applies to every remaining value. When matching a foreign `#[non_exhaustive]` enum, prefer explicitly naming all known variants and retain a sensible wildcard arm for future variants. List known variants with identical fallback behavior in explicit arms, then keep a separate `_` arm for unknown variants. This avoids the redundant wildcard-or pattern that Clippy discourages. This documents today's cases; the wildcard still means new variants need not cause a compilation error. Do not claim exhaustive future checking.

Use `if let` or `let ... else` when only particular variants matter. A `match` that names just those variants and uses a wildcard is acceptable when it is clearer for that limited purpose. Do not omit a meaningful case merely to silence the compiler.

Use `matches!` for a simple predicate. Use a match guard when it keeps a condition close to the case it refines. Move substantial work out of patterns and guards so the selection logic remains readable.

### Prefer simple expressions and early exits

Use an `if` or `match` expression to choose a value instead of declaring an uninitialized binding and assigning it in each branch. Use early returns for invalid inputs and completed cases; keep the main operation at a shallow indentation level.

Use the tail expression for a function's final result. Reserve `return` for an early exit or a case where it clarifies the control flow. Do not add `return` to every match arm.

Avoid nested ternary-like expressions, long chains of `&&` and `||` with hidden side effects, and boolean comparisons such as `condition == true`. Name a compound condition when the name explains a domain rule.

Do not make an expression artificially short by hiding a mutation or failure in a helper with an innocent name.

### Use combinators when they state the operation clearly

Use `map`, `and_then`, `ok_or`, `transpose`, and related methods when they communicate a straightforward transformation. Use `match` when branches need explanation, several statements, or distinct effects.

Choose the lazy form when constructing a fallback does meaningful work: `unwrap_or_else`, `ok_or_else`, or `or_else`. Use the simpler eager form for an already available value or a trivial error constructor.

Do not turn a failed operation into absence with `.ok()` unless discarding the error category is intentional. Do not use `unwrap_or_default()` to silently accept malformed data or conceal a required value.

### Add types where inference hides a decision

Let the compiler infer obvious local types. Add an annotation when parsing, conversion, or collection construction would otherwise leave the reader guessing.

```rust
use std::num::ParseIntError;

fn parse_u32_le(input: &str) -> Result<[u8; 4], ParseIntError> {
    let value: u32 = input.parse()?;

    Ok(value.to_le_bytes())
}
```

The annotation makes the parsed integer's width explicit before its conversion to bytes. Do not add a binding only to repeat a function's return type.

Prefer annotations on bindings over turbofish syntax, especially for generic returns such as `.into()` or `.parse()` and complex iterator-chain results. Use a turbofish when inference requires it or that location is materially clearer; `_` may stand in for inferable parts. Avoid long nested type expressions in the middle of a method chain; introduce a named type or intermediate binding.

## Iteration and collections

### Iterate over the values you need

Use element iteration for element operations. Use `enumerate` when the index is also needed, `zip` for corresponding sequences, and slices or chunks when they express the algorithm.

Use indexing when positions are essential to the algorithm. Use checked access such as `get` when an index can be invalid under the function's normal contract. Direct indexing is appropriate when the surrounding code establishes the bounds.

Choose ownership explicitly:

| Form for a `Vec<T>`      | What the loop receives                 |
| ------------------------ | -------------------------------------- |
| `for item in &items`     | Shared references                      |
| `for item in &mut items` | Mutable references                     |
| `for item in items`      | Owned elements; the vector is consumed |

Do not write `0..items.len()` just to index every element. Do not use `zip` as a length check: it stops when either input ends. Validate equal lengths separately when the operation requires them. [Iterator methods][iterator]

### Use chains for transformations and loops for procedural work

A short chain is appropriate for a clear transformation, filter, or query:

```rust
#[must_use]
fn contains_negative(values: &[i32]) -> bool {
    values.iter().any(|&value| value < 0)
}
```

Use a `for` loop when the body has several effects, meaningful branching, an early exit, or error handling that is easier to follow as statements. Do not use `map` for side effects or collect a temporary vector only to force an iterator to run.

Prefer a loop to `for_each` unless there is a concrete reason to use the latter. Bind iterator adapter chains to a named variable before consuming them in a `for` loop; do not build the chain in the loop header. A simple loop over `&items`, `&mut items`, or `items` needs no extra binding.

Keep closures short enough that the enclosing operation remains apparent. Move a substantial closure body into a named function when it has an independent meaning. Do not impose a fixed chain length; clarity depends on the operations, not the number of dots.

### Preserve failures while collecting

Collect an iterator of results into `Result<Vec<_>, _>` when any failure should fail the operation. Use `filter_map(Result::ok)` only when intentionally skipping failures is the contract and the lost information does not matter.

```rust
use std::num::ParseIntError;

fn parse_all(input: &[&str]) -> Result<Vec<u32>, ParseIntError> {
    input.iter().map(|value| value.parse()).collect()
}
```

If the operation must collect both successes and failures, write that explicitly. Do not disguise partial success as complete success. [Iterator collection][iterator]

### Use collection operations that express the intent

Use `entry` for a lookup followed by conditional insertion or modification. Use `get_mut` when updating an existing entry without insertion. Avoid repeated lookups when a single operation states the intent more clearly.

```rust
use std::collections::HashMap;

#[must_use]
fn count_words(words: &[String]) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();

    for word in words {
        *counts.entry(word.as_str()).or_insert(0) += 1;
    }

    counts
}
```

Choose ordered collections or sort explicitly when ordering is part of the result. Do not rely on `HashMap` or `HashSet` iteration order. Prefer standard collection types when their semantics fit the operation. [Standard collections][collections]

## Errors with thiserror and anyhow

### Choose the error type for the caller

Return a concrete error when callers need to distinguish failures. Use `anyhow::Result<T>` when the surrounding code's job is to add context and report an operation's failure.

| Contract                                              | Return type                        |
| ----------------------------------------------------- | ---------------------------------- |
| A value may be absent                                 | `Option<T>`                        |
| Failure cases are part of the interface               | `Result<T, SpecificError>`         |
| A standard error fully describes the operation        | For example, `io::Result<T>`       |
| An operation may fail or find no value                | `Result<Option<T>, SpecificError>` |
| Coordination or reporting needs an error with context | `anyhow::Result<T>`                |

Apply this distinction within a binary as well as across crates. Parsing, validation, and other reusable operations still need meaningful contracts when they happen to live inside an application.

Do not return `String`, `&str`, or `()` as the error contract of a reusable fallible API. A small named error is enough for a single failure category. Do not introduce one project-wide enum that accumulates every unrelated error.

A module-local `type Result<T> = std::result::Result<T, DecodeError>` is acceptable when it makes repeated signatures clearer. Keep aliases local and explicit enough that readers can identify the error type. Keep `anyhow::Result` qualified when mixing it with typed results in one file.

### Define ordinary error types with thiserror

Use `thiserror` for project-owned error types when external dependencies are appropriate. It generates ordinary implementations of the standard error traits. A manual implementation is acceptable for a concrete target or dependency constraint; preserve the same `Debug`, `Display`, and `Error` contract.

Variants should describe failures the caller can understand or act on. Give fields meaningful names. Keep messages concise, normally lowercase without a final period, and include the facts that distinguish the failure. Do not expose secret or irrelevant input in a message.

This complete example preserves an integer parsing error and adds a separate validation case:

```rust
use std::num::{NonZeroUsize, ParseIntError};

#[derive(Debug, thiserror::Error)]
enum ParseLimitError {
    #[error("limit is not a valid integer")]
    InvalidInteger(#[from] ParseIntError),

    #[error("limit must be greater than zero")]
    Zero,
}

fn parse_limit(input: &str) -> Result<NonZeroUsize, ParseLimitError> {
    let value: usize = input.parse()?;
    NonZeroUsize::new(value).ok_or(ParseLimitError::Zero)
}
```

The `#[from]` conversion allows `?` to wrap `ParseIntError` and also identifies it as the source. Use this only when the conversion is unambiguous and requires no additional contextual fields. [thiserror][thiserror]

### Preserve sources and contextual fields

Retain an underlying error as an error. Do not replace it with `to_string()` or `format!` merely to propagate it. Use `#[source]`, a field named `source`, or `#[from]` as appropriate.

When wrapping needs an offset, identifier, or operation name, construct the error explicitly. This alternative parser illustrates a contextual wrapper:

```rust
use std::num::ParseIntError;

#[derive(Debug, thiserror::Error)]
#[error("field {index} is not a valid integer")]
struct FieldError {
    index: usize,
    #[source]
    source: ParseIntError,
}

fn parse_field(input: &str, index: usize) -> Result<u32, FieldError> {
    input.parse().map_err(|source| FieldError { index, source })
}
```

A nontransparent wrapper's display message should describe its own context. Do not repeat `{source}` when the same source is already exposed in the chain. Use `#[error(transparent)]` when the wrapper intentionally delegates its presentation and source behavior to the contained error. [Error sources][error] [thiserror attributes][thiserror]

Do not name a non-error field `source` on a derived error; use `input`, `origin`, or another accurate name. Keep public error fields and variants as deliberate API choices. Exposing a dependency's error type is still an API dependency even when the derive macro is an implementation detail.

### Propagate with question mark and add useful context

Use `?` when the right behavior is propagation. Use `match` to recover or distinguish categories. Use `map_err` for an intentional error conversion or contextual wrapper. Avoid a verbose match that only reconstructs `Ok` and `Err` unchanged.

Add `anyhow` context where the code knows what the attempted operation means. This snippet uses `parse_limit` and `NonZeroUsize` from the limit-parsing example:

```rust
fn configured_limit(input: &str) -> anyhow::Result<NonZeroUsize> {
    use anyhow::Context as _;

    parse_limit(input).context("reading the configured item limit")
}
```

Use `.context("fixed explanation")` for fixed text and `.with_context(|| format!(...))` when constructing context should happen only on failure. Each layer should add information the lower layer could not know. Do not attach "operation failed" repeatedly.

`anyhow` preserves the underlying error; it does not merely turn it into a string. Context also preserves the ability to downcast to the underlying error. Even so, repeated downcasting for ordinary control flow is a sign that the function should return a typed error. [anyhow Context][context]

Use `anyhow::bail!` and `anyhow::ensure!` for failures in an `anyhow::Result` function. Keep them out of functions whose contract requires a specific project error. Do not replace structured categories with message matching.

### Handle a failure or return it

A function that merely passes an error upward should not also report that same failure. Report it at the point that takes responsibility for presenting or handling it. Recovery may deserve its own message when it describes a distinct action.

Do not discard a `Result` with `let _ =`, `.ok()`, or an empty `Err` branch without an intentional policy. When ignoring a failure is correct, make the reason visible. Do not add an elaborate wrapper just to suppress a diagnostic.

## Panics and assertions

### Keep expected failures fallible

Use `Result` for malformed input and expected operational failures that need diagnostic context. Use `Option` for normal absence or a simple failure whose cause is already clear. Reserve panics for defects, violated invariants, or failure to satisfy a documented precondition. A codebase may adopt a stricter no-panic policy and return errors for those cases too.

Do not use production `unwrap()` or `unwrap_err()`. Prefer a pattern, `?`, or a meaningful fallback. If an invariant establishes success and a panic is appropriate, use `expect` with a message stating that invariant.

```rust
#[must_use]
fn active_name(names: &[String], active_index: usize) -> &str {
    names
        .get(active_index)
        .expect("active_index is maintained within the names collection")
}
```

This is an internal helper example: its caller must maintain the stated invariant. If arbitrary callers can supply an invalid index as a normal input, change the interface to return `Option` or `Result` instead. Writing an assertion message does not establish the invariant by itself.

Use `unreachable!` only when control flow or a maintained invariant rules the case out. Do not use it as a fallback for unfamiliar input or a newly added enum variant. [Panic guidance][panic]

### Make assertions explain the broken relationship

Use `assert_eq!` or `assert_ne!` for comparisons when possible. Always consider a contextual message, especially when the assertion would otherwise need a comment, users may see the failure, diagnostic values matter, or it must distinguish caller misuse from an implementation bug. The message should explain the expected relationship and add more than a restatement of the condition.

A test or invariant-check excerpt:

```rust
assert_eq!(
    index.len(),
    records.len(),
    "rebuilding the index should create one entry for each record"
);
```

Use `debug_assert!` only for checks that may be omitted. Do not put required validation or necessary side effects inside it.

Remove reachable `todo!`, `unimplemented!`, and temporary panic placeholders from accepted code.

Tests may use `unwrap`, `expect`, and assertions when failure should fail the test. Examples intended for users should normally demonstrate the error-handling style those users should adopt.

Developer-facing binaries may panic for otherwise recoverable failures when terminating the tool is an appropriate response. Prefer ordinary errors when useful reporting or recovery is needed. The production `unwrap` restriction still applies.

### Restrict panic recovery to an optional defensive boundary

Do not call `std::panic::catch_unwind` in first-party code except as defense in depth at a critical entry point in code already making a serious effort to avoid panics. Review must establish those existing precautions. Do not rely on the catch for correctness, soundness, required cleanup, or another guaranteed property.

This exception requires an environment where unwinding is possible. It does not apply to builds that require `panic = "abort"`. An aborting panic bypasses `catch_unwind`. Use `Result` for expected failures even when a defensive panic boundary exists. [catch_unwind][catch-unwind]

## Concurrency

### Do not recover data from poisoned mutexes

Do not obtain or inspect the protected contents of a poisoned mutex, including through `PoisonError::into_inner`, `get_ref`, or `get_mut`. Do not clear poison to bypass this rule. Report the failed operation or stop at an appropriate failure boundary instead of continuing with potentially inconsistent state.

This is a project policy about recovery. Rust mutex poisoning is advisory, and some panics may not poison a mutex. Never rely on poisoning to enforce a safety invariant. [Mutex poisoning][mutex]

Choose synchronization primitives and scheduling facilities appropriate to the codebase.

## Async code and Tokio

Apply this section when using async Rust and Tokio.

### Give async functions ordinary Rust interfaces

Use `async fn` for a named asynchronous operation and an async block for short local composition. Keep ordinary transformations synchronous unless an interface requires an async form. Follow the same naming, visibility, ownership, and error rules as synchronous functions.

Do not append `_async` to every name. Use a distinguishing suffix only when it clarifies an intentionally paired interface. Accept borrowed inputs when ownership is unnecessary; being async does not by itself require owned arguments.

```rust
use std::{io, path::Path};

async fn load_lines(path: &Path) -> io::Result<Vec<String>> {
    let text = tokio::fs::read_to_string(path).await?;

    Ok(text.lines().map(str::to_owned).collect())
}
```

This example uses Tokio's `fs` feature. Choose async operations for async code; do not quietly replace them with blocking calls inside an `async fn`.

### Await directly when the work belongs to the current operation

Call `.await` directly when the surrounding operation needs the result. Do not introduce a spawned task merely to invoke an async function. Propagate ordinary failures with `.await?` and attach context where it explains the operation.

Use `async move` when the future should own its captures. Moving a reference moves the reference, not the referent, and does not make borrowed data `'static`.

Add `Send`, `Sync`, and `'static` bounds only when the interface requires them. `tokio::spawn` requires a `Send + 'static` future and output; a directly awaited future does not inherit those requirements merely because Tokio is in use. [Tokio spawn][spawn]

### Keep the two error layers of a task visible

Give a spawned task's handle a descriptive name and an intentional owner. Observe its result unless detachment is explicitly part of the operation's contract. Dropping a Tokio `JoinHandle` detaches the task. [JoinHandle][joinhandle]

A task returning `Result<T, E>` introduces both a join result and the operation's result. Handle both. This helper accepts an already created task:

```rust
use std::io;

use tokio::task::JoinHandle;

async fn finish_load(load_task: JoinHandle<io::Result<String>>) -> anyhow::Result<String> {
    use anyhow::Context as _;

    let result = load_task.await.context("joining the loader task")?;
    result.context("loading the text")
}
```

The helper uses Tokio's `rt` feature and consumes the handle. Cancelling the helper before it finishes drops the handle and detaches the task; use this signature only when that consequence is intentional. A concise `.await??` is acceptable when both conversions and their meaning are clear; separate the layers when they need different context.

Keep runtime construction at an explicit application or embedding boundary. Reusable helpers should expose async operations; do not put `#[tokio::main]` on ordinary helpers or call `block_on` from within an ordinary async function. [Tokio runtime integration][bridging]

## Text paths and numbers

### Distinguish text from bytes and paths

Use `str` and `String` for UTF-8 text, `[u8]` and `Vec<u8>` for bytes, and `Path` and `PathBuf` for filesystem paths. Use `OsStr` or `OsString` when an operating-system string is the actual value.

Do not require a path to be UTF-8 merely to pass it through the program. Join paths with path operations instead of inserting separators into strings. Make lossy conversion an explicit presentation decision, not a silent change to an identifier.

Distinguish byte offsets, Unicode scalar values, and user-perceived characters. `str::len()` counts bytes. A byte offset is not automatically a valid string boundary. Use text operations that express the intended unit. [str][str] [Path][path]

### Use formatting for readable text construction

Use `format!` for a formatted owned string and `write!` when writing formatted text into a destination. Prefer them over chained `+` when combining more than two strings, and over `str::replace` when substituting values into a string literal. Prefer captured arguments such as `format!("item {index}")` when they keep the message clear. Use `push` and `push_str` for straightforward incremental assembly.

Use `Display` for the type's intended textual presentation and `Debug` for diagnostics. Do not treat a derived debug representation as a stable format for serialization or identifiers. [Debug formatting][debug]

### Make numeric meaning and conversion explicit

Use `usize` for ordinary in-memory lengths and indices. Use a fixed-width integer when a format or value range requires that width. Use `Duration` for a time span instead of an undocumented integer count.

Prefer `From` and `TryFrom` to an `as` cast that can truncate, change sign, or lose precision. If a cast intentionally has those effects, make the policy or range argument visible.

```rust
use std::num::TryFromIntError;

fn encode_length(len: usize) -> Result<[u8; 4], TryFromIntError> {
    let len = u32::try_from(len)?;
    Ok(len.to_le_bytes())
}
```

Choose checked, saturating, or wrapping arithmetic when the distinction matters to the operation. Do not rely on a particular build mode to define the intended overflow behavior. For floating-point values, handle NaN, infinities, and comparison according to the operation's meaning; do not mechanically substitute an epsilon comparison everywhere. [Numeric conversions][casts] [Overflow][overflow]

## Documentation and comments

### Document the contract where it is declared

Use `///` for item documentation and `//!` at the top of a file for a module or crate overview. Write Markdown and place documentation before the item's attributes. Prefer directly written doc comments to macro-generated documentation so the source remains readable. Describe what the caller needs to know: the operation, input interpretation, output, failure conditions, ownership implications, and any relevant invariants.

Document public APIs; consider `missing_docs` to help enforce coverage. Document private items when their purpose or assumptions are not obvious. Do not add boilerplate that merely repeats the signature or explains basic Rust syntax. A self-explanatory enum variant may omit its own documentation, and a narrow lint exception is appropriate when a comment would say nothing useful.

Use `# Errors`, `# Panics`, and `# Safety` sections when applicable. Always document panic conditions caused by malformed inputs, and state when an error occurs. For unsafe functions and traits, explain the obligations required to use or implement them safely. Link related items with rustdoc links. [API documentation][documentation]

### Follow ordinary Rust comment formatting

Prefer line comments to block comments. Put one space after `//`, `///`, or `//!`, and normally write complete sentences with capitalization and punctuation. Put a comment on its own line when practical; use one space before a trailing comment. Inline block notes may omit sentence punctuation.

Limit comment-only lines to 80 characters excluding indentation, while also keeping the whole line within the 100-character code width. Use ordinary `//` comments for implementation details and private TODOs even beside public documentation; mixing comment kinds is appropriate when they serve different audiences. [Official comments and attributes][style]

### Explain decisions in implementation comments

Use `//` for a local explanation of an algorithm, invariant, unusual ordering, or constraint. Explain why a surprising choice is necessary. Remove comments that merely translate the next statement into English.

Put an assertion's useful failure explanation in its message so the explanation accompanies the failure. Put a safety argument immediately beside the unsafe operation it justifies.

Do not leave commented-out implementations in the source. A TODO should describe a specific remaining change and the reason it matters; it must not be the only specification of behavior the current code needs to provide.

### Keep examples representative

Show the intended way to use an API, including ordinary error propagation. Cover enough of the crate's functionality for readers to understand how to use it; every function does not need its own example. Prefer small examples with meaningful names and enough context to understand the code. Avoid examples that require unrelated setup before the API becomes visible.

Keep documentation examples valid and executable where practical. Use `compile_fail` for an intentional compile-failure example. Use `no_run` when a valid example should compile but needs an unavailable execution environment. Do not use `ignore` merely to conceal a broken example. [Rustdoc examples][doctests]

## Tests

Use the repository's established test framework. Use `#[test]` for ordinary unit tests and the runtime's test attribute for async tests unless the repository requires another framework.

### Write tests that state the behavior

Prefix every test function with `test_`, then name the case and expected outcome: `test_rejects_zero_batch_size`, `test_preserves_empty_fields`, or `test_returns_none_for_unknown_name`. This applies to unit, integration, and async test functions. Ordinary test helpers do not need the prefix. Keep setup local and make the assertion easy to find.

Prefer a small number of direct assertions about one behavior. Use table-driven cases when the inputs share the same contract, and include enough case context in failures to identify the input. Avoid a test helper that hides both the action and the assertion behind a generic name.

Keep unit tests beside the implementation in `#[cfg(test)] mod tests`. Use integration tests for behavior through the crate's public interface. Do not expand production visibility merely to inspect an implementation detail. [Test organization][test-organization]

The following tests belong beside the earlier `BatchSize` example:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preserves_positive_batch_size() -> Result<(), InvalidBatchSize> {
        let size = BatchSize::try_from(8)?;

        assert_eq!(size.get(), 8);
        Ok(())
    }

    #[test]
    fn test_rejects_zero_batch_size() {
        assert_eq!(BatchSize::try_from(0), Err(InvalidBatchSize));
    }
}
```

### Assert the contract instead of incidental representation

Compare structured values and error variants when those are the contract. Do not assert an error's entire display string or a type's debug output unless that exact text is intentionally the behavior under test.

Include relevant boundary and failure cases. Test an invariant's observable consequences; do not repeat the implementation's internal steps in a second function and compare the two.

Keep inputs and ordering deterministic where the test does not concern randomness or ordering itself. Do not assume a hash collection's iteration order. For async tests, use the runtime's test facilities and explicit completion signals; an arbitrary sleep is not proof that another operation completed.

## Attributes macros and unsafe code

### Keep attributes narrow and meaningful

Place each attribute on its own line at the item's indentation, after documentation comments. Prefer outer attributes where possible. Combine derives into one `#[derive(...)]` attribute, preserving order when changing it could affect macro behavior; do not impose an automatic alphabetical reorder. Keep conditional compilation easy to see. Prefer one coherent conditional module to repeating a complicated condition across many unrelated items. [Official attributes][style]

Use `#[cfg(...)]` when code must be excluded from a build. `cfg!(...)` produces a boolean expression; both branches of an ordinary `if` still have to type-check. [Conditional compilation][cfg]

Keep lint exceptions on the smallest useful item or block and include a reason. Prefer `#[expect(..., reason = "...")]` when an expected diagnostic should remain present; use `#[allow(..., reason = "...")]` when a justified scope may legitimately stop producing it. Do not silence warnings across a whole crate for convenience. [Diagnostic attributes][diagnostics]

### Mark results that must be used

Apply `#[must_use]` to a function when ignoring its return value prevents correct use. This includes pure computations and functions returning errors or other critical status information. An additional function annotation is unnecessary when the return type already carries the attribute, as `Result` does. Add a useful message when the reason is not apparent.

Apply `#[must_use]` to types that carry this obligation across their producing functions, including result-like error carriers and lazy computations whose intended work happens only when the value is consumed or polled. Mark the shared type rather than repeating the annotation on every producer. Do not use the attribute indiscriminately for values that callers may correctly discard.

### Use macros for syntax that earns its complexity

When designing an API or generating code, prefer these mechanisms in order:

1. Ordinary Rust functions, types, traits, objects, and modules.
2. Declarative `macro_rules!` macros.
3. Other reproducible code generation, such as a build-system generator.
4. Procedural macros.

Use metaprogramming only when its benefit justifies harder IDE navigation, less obvious expansion behavior, reduced cross-project familiarity, and more difficult automated refactoring. A declarative macro is appropriate for necessary syntactic repetition or an interface ordinary functions cannot express. Established standard macros and derives, including this guide's `thiserror` convention, remain useful tools; this preference order concerns choosing a mechanism, not replacing every existing derive with handwritten code.

Keep a custom macro's expansion predictable. Do not hide a surprising `return`, `.await`, panic, or repeated evaluation of an argument. Document behavior that differs from an ordinary function call. Use `$crate` for references into the defining crate when writing an exported declarative macro. [Declarative macros][macros]

### Isolate and explain unsafe operations

Do not write unsafe code when equivalent required functionality can be implemented safely. Encapsulate necessary unsafe operations behind a sound safe abstraction and isolate them from business logic. Prefer a small separate crate with few dependencies beyond the standard library when the boundary warrants one. Keep each unsafe block as small as practical. Give it a `// SAFETY:` comment explaining why the required conditions hold at that operation.

Also provide a technical justification for using unsafe code, at the block, file, or target level as appropriate. Explain why an equivalent safe implementation is insufficient. If the justification depends on performance, provide benchmarks that demonstrate the relevant constraint and benefit; an unsupported claim that unsafe code is faster is not enough.

An unsafe function still needs explicit unsafe blocks around unsafe operations. Its `# Safety` documentation explains the caller's obligations; the local comments explain how the implementation satisfies the operations it performs.

Treat the safe code that maintains an unsafe abstraction's invariants as part of its safety argument. Do not assume that a safe function inside the same module is irrelevant to soundness. Keep unsafe trait implementations, attributes, and foreign declarations equally explicit about their obligations. [Unsafe abstractions][nomicon] [Unsafe operations in unsafe functions][unsafe-op]

## Language and API boundaries

### Use stable language and library features

Do not use unstable language or library features. Use the repository's supported stable toolchain and avoid feature gates added for convenience. A formatter option's availability is a separate tooling question.

### Respect hidden dependency APIs

Treat another owner's `#[doc(hidden)]` items as private, even if Rust visibility makes them callable. Do not depend on them without owning that code. A public signature alone is not an invitation to use an intentionally hidden implementation interface.

### Treat public const functions as a commitment

Make a public function `const` only when it is expected to remain callable in constant evaluation, or when its owner commits to migrating affected clients if that changes. Removing `const` can break callers even when the runtime behavior is unchanged.

### Make runtime initialization explicit

Do not arrange for project code to execute before `main`, for example through the `ctor` crate or equivalent startup hooks. Initialize resources through an explicit application entry point or embedding interface. Compile-time evaluation of constants is separate from runtime code that executes before the entry point.

## References

These references explain the language and crate behavior used above. The coding rules in this document are project policy; a style requirement is not necessarily a Rust language requirement.

- [Rust Style Guide][style] and [Rust API Guidelines][api] for formatting, naming, and API conventions.
- [Rust Reference][reference] and [standard library documentation][std] for language rules and type contracts.
- [thiserror][thiserror] for derived error implementations and source handling.
- [anyhow][anyhow] and [Context][context] for contextual errors at reporting and coordination boundaries.
- [Tokio][tokio] for async operations, task handles, and runtime integration.

[style]: https://doc.rust-lang.org/style-guide/
[api]: https://rust-lang.github.io/api-guidelines/
[naming]: https://rust-lang.github.io/api-guidelines/naming.html
[flexibility]: https://rust-lang.github.io/api-guidelines/flexibility.html
[interoperability]: https://rust-lang.github.io/api-guidelines/interoperability.html
[documentation]: https://rust-lang.github.io/api-guidelines/documentation.html
[reference]: https://doc.rust-lang.org/reference/
[std]: https://doc.rust-lang.org/std/
[modules]: https://doc.rust-lang.org/reference/items/modules.html
[visibility]: https://doc.rust-lang.org/reference/visibility-and-privacy.html
[nonexhaustive]: https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute
[traits]: https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
[from]: https://doc.rust-lang.org/std/convert/trait.From.html
[tryfrom]: https://doc.rust-lang.org/std/convert/trait.TryFrom.html
[borrow]: https://doc.rust-lang.org/std/borrow/trait.Borrow.html
[deref]: https://doc.rust-lang.org/std/ops/trait.Deref.html
[clone]: https://doc.rust-lang.org/std/clone/trait.Clone.html
[option]: https://doc.rust-lang.org/std/option/enum.Option.html
[take]: https://doc.rust-lang.org/std/mem/fn.take.html
[iterator]: https://doc.rust-lang.org/std/iter/trait.Iterator.html
[collections]: https://doc.rust-lang.org/std/collections/index.html
[thiserror]: https://docs.rs/thiserror/latest/thiserror/
[anyhow]: https://docs.rs/anyhow/latest/anyhow/
[context]: https://docs.rs/anyhow/latest/anyhow/trait.Context.html
[error]: https://doc.rust-lang.org/std/error/trait.Error.html
[panic]: https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html
[tokio]: https://docs.rs/tokio/latest/tokio/
[spawn]: https://docs.rs/tokio/latest/tokio/task/fn.spawn.html
[joinhandle]: https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html
[bridging]: https://tokio.rs/tokio/topics/bridging
[str]: https://doc.rust-lang.org/std/primitive.str.html
[path]: https://doc.rust-lang.org/std/path/struct.Path.html
[debug]: https://doc.rust-lang.org/std/fmt/trait.Debug.html
[casts]: https://doc.rust-lang.org/reference/expressions/operator-expr.html#type-cast-expressions
[overflow]: https://doc.rust-lang.org/reference/expressions/operator-expr.html#overflow
[doctests]: https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html
[test-organization]: https://doc.rust-lang.org/book/ch11-03-test-organization.html
[cfg]: https://doc.rust-lang.org/reference/conditional-compilation.html
[diagnostics]: https://doc.rust-lang.org/reference/attributes/diagnostics.html
[macros]: https://doc.rust-lang.org/reference/macros-by-example.html
[nomicon]: https://doc.rust-lang.org/nomicon/working-with-unsafe.html
[unsafe-op]: https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-op-in-unsafe-fn.html
[style-items]: https://doc.rust-lang.org/style-guide/items.html
[style-expressions]: https://doc.rust-lang.org/style-guide/expressions.html#enum-literals
[rustfmt-config]: https://rust-lang.github.io/rustfmt/
[catch-unwind]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
[mutex]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#poisoning
