# This script takes care of testing your crate

set -ex

# TODO This is the "test phase", tweak it as you see fit
main() {
    cargo check
    cargo clippy -- -D warnings
    cross build --target $TARGET
    cargo test --target $TARGET -- --test-threads=1
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
