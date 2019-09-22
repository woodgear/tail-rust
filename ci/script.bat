set TARGET=%1
cargo check
cargo clippy -- -D warnings
cross build --target %TARGET%
cargo test --target %TARGET% -- --test-threads=1
