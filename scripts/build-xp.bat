set RUSTFLAGS=-C target-feature=+crt-static  -C link-arg=/SUBSYSTEM:CONSOLE,5.01 
cargo build --release 
