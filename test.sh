echo "ok"
rm -rf ./target/debug/deps/*tokio_io*
rm -rf ./target/debug/deps/*tokio_fs*
rm -rf ./target/debug/deps/*tokio_codec*
# rm -rf ./target/debug/deps/*tokio

cargo test test_simple -- --nocapture