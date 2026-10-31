# TEST_FEATURES := ""
export RUSTFLAGS := "-Awarnings"

format:
    just --fmt --unstable

    cargo fmt

check:
    #!/usr/bin/env bash
    set -euxo pipefail

    cargo check --workspace 
    cargo check --workspace --tests
    cargo check --workspace --examples

[arg("fix", long="fix", value="true")]
[arg("package", long="package")]
lint fix="false" package="":
    #!/usr/bin/env bash
    set -euxo pipefail

    disallow=(
        warnings
        deprecated
        unused_extern_crates
        unused_import_braces
        unused_qualifications
        clippy::clone_on_ref_ptr
        clippy::all
        clippy::correctness
        clippy::suspicious
        clippy::complexity
        clippy::perf
        clippy::style
        clippy::absolute_paths
        clippy::pedantic
        clippy::nursery
    )
    allow=(
        missing_docs
        clippy::missing_errors_doc
        clippy::missing_panics_doc
        trivial_casts
        trivial_numeric_casts
        clippy::cast_sign_loss
        clippy::cast_precision_loss
        clippy::assert_is_empty
        clippy::too_many_lines
        clippy::float_cmp
        clippy::too_long_first_doc_paragraph
        clippy::too_many_arguments
        clippy::missing_const_for_fn
    )


    if [ "{{ fix }}" = "true" ]; then
        cargo clippy --no-deps "${scope[@]}" --fix --allow-dirty \
            -- ${disallow[@]/#/-D } ${allow[@]/#/-A }
    else
        cargo clippy --no-deps "${scope[@]}" \
            -- ${disallow[@]/#/-D } ${allow[@]/#/-A }
    fi

test:
    #!/usr/bin/env bash
    set -euxo pipefail

    cargo test --workspace --lib --bins --tests -- --nocapture
    cargo test --workspace --doc -- --nocapture
