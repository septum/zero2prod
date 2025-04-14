_default:
    @just --list
    @echo You probably want to run \`setup\` first.

setup:
    rustup component add rustfmt
    rustup component add clippy
    rustup component add llvm-tools-preview
    cargo install cargo-watch
    cargo install cargo-llvm-cov
    cargo install cargo-audit

dev:
    cargo watch -x check -x test -x run

test:
    cargo test

format:
    cargo fmt -- --check

lint:
    cargo clippy -- -D warnings

coverage:
    cargo llvm-cov

audit:
    cargo audit
