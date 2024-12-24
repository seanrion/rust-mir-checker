cargo fmt --all
cargo clippy --no-default-features --all-targets -- -A clippy::non-send-fields-in-send-ty -D warnings 
# cargo build --release