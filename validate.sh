cargo fmt --all
cargo check --all-targets
cargo clippy --no-default-features --all-targets -- -D warnings
