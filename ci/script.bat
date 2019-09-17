set TARGET=%1
cargo check
cargo check
cross build --target %TARGET%
cargo test --target %TARGET% -- --test-threads=1
