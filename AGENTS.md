# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

The MLIR Toy tutorial (chapters 1–7) implemented in Rust on melior 0.28 (MLIR C API, LLVM/MLIR 23).

## Commands

All commands need the Nix dev shell (`nix develop`, or direnv via `.envrc`): it provides LLVM/MLIR 23 and sets `MLIR_SYS_230_PREFIX`, without which `mlir-sys` fails to build.

```sh
just test                      # all tests (lib, bins, integration, doc)
just lint                      # clippy with pedantic + nursery as errors (melior-ext and toy only)
cargo xtask no-slop            # project-specific style scanner (must report 0 findings)
cargo fmt

cargo test -p toy --test chapter5                  # one chapter's integration tests
cargo test -p toy --lib dialect::tests::test_name  # one unit test
cargo run -- examples/codegen.toy --emit=jit [--opt]
```

`--emit` is one of `ast`, `mlir`, `mlir-affine`, `mlir-llvm`, `llvm`, `jit`. `toyc` also reads MLIR input (`.mlir` files or `-x mlir`), e.g. its own `--emit=mlir` output.

## Architecture

- `toy/` — the compiler library plus the `toyc` binary. `toy/src/lib.rs` maps tutorial chapters to modules; `toy/src/compiler.rs` builds the pass pipeline (`run_pipeline`), which is the best entry point for understanding the flow: parse → `mlir_gen` → inline + symbol-dce → canonicalize/shape-inference/CSE → toy-to-affine → print-to-llvm + builtin conversion passes → LLVM IR / JIT.
- `melior-ext/` — safe wrappers for C API features melior lacks (extensible-dialect traits, memory-effect interfaces, dynamic types, rewrite/conversion patterns, JIT, LLVM IR translation). **All `unsafe` code belongs here**; `toy` is safe Rust.
- `xtask/` — the vendored no-slop scanner; not a default workspace member and not linted by `just lint`.
- `toy/tests/chapterN.rs` pin the exact IR/output of each chapter; `examples/` holds the Toy programs they use.

### The Toy dialect is defined at runtime

There is no ODS/TableGen. `toy/src/dialect/toy.irdl.mlir` declares ops and the `!toy.struct` type in IRDL; `dialect::load` loads it and then attaches from Rust the terminator trait, `Pure` (via memory-effect/speculatability fallback models), and per-op verifiers (`dialect/verifiers.rs`). `dialect/ops.rs` holds typed op wrappers (`AddOp::build`, `AddOp::cast`, `ToyOp::defining`) standing in for ODS-generated classes. Consequences to keep in mind:

- Toy ops print in MLIR's generic form; struct types print as `!toy.struct<[...]>`.
- IRDL constraint SSA values are *variables*: every use within an op must match the same type, including all elements of a variadic list. So each operand/result needs its own constraints, and heterogeneous variadic ops are impossible — hence calls are `func.call` with `toy.cast` inserted by `mlir_gen`, and functions are `func.func` (no `toy.generic_call`/`toy.func`).
- Attached trait verifiers run *before* the IRDL constraints, so verifiers must return `Ok` on malformed structure and let IRDL report it.
- MLIR's canonicalizer and inliner can't use Toy patterns/interfaces through the C API, so `transforms/` contains a Toy canonicalization pass (greedy driver over Rust patterns), a hand-written inliner, and shape inference via the `dialect::ShapeInference` trait.

### Working with melior / the C API

- Need a `&Context` inside a pass or pattern? Use `melior_ext::with_context` or the `context` parameter that pattern callbacks receive. Never derive one from `ContextRef::to_ref()` and return it — it points at the temporary `ContextRef`.
- Pattern callbacks (`op_rewrite_pattern`, `ConversionPatternSet::add`) take `(&Context, OperationRef, [operands,] RewriterBase)` and return `melior_ext::LogicalResult`; all IR changes in patterns must go through the rewriter.
- Dialect conversion legalizes ops created by patterns, so their dialects must be marked legal even in partial conversions.
- Known melior gaps: `previous_in_block()` panics (`todo!`), `walk` callbacks can't escape refs (use `melior_ext::nested_operations`), and the ODS `llvm.call` builder sets wrong `operandSegmentSizes` (build it with `OperationBuilder`).
- Pass identity uses `TypeId::create(&STATIC)` with a distinct `static PASS_ID: u64` per pass.

## Conventions

Follow `RUST_STYLE_GUIDE.md` (binding). Notable points enforced in this repo:

- Grouped imports (`std`, external, `crate::`) with no inline path qualification; import modules for free functions.
- Module trees use `name.rs` + `name/` (not `mod.rs`; the no-slop `require-mod-rs-layout` rule is disabled for this reason).
- Test functions are prefixed `test_`; thiserror messages are lowercase; fallible constructors are named `create`, not `new`.
- Every `unsafe` block has a `// SAFETY:` comment; lint exceptions use `#[expect(..., reason = "...")]`.
- Doc comments put one sentence per line (no-slop `require-sentence-per-line`).
- Comments explain concepts the way the MLIR tutorial does — the code is meant to be read as a learning resource.
