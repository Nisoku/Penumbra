build:
    cargo build

check:
    cargo check

test:
    cargo test

format:
    cargo fmt --all -- --check

format-fix:
    cargo fmt --all

clippy:
    cargo clippy --all-targets -- -D warnings

all: check format clippy test

docs:
    cargo doc --no-deps --all-features
