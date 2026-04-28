# Agent Quality Gates

Any code change in this repository must pass all of the following:

1. `cargo fmt --all -- --check`
2. `cargo check --all-targets --all-features`
3. `cargo clippy --all-targets --all-features -- -D warnings`
4. `cargo test --all-targets --all-features`

Do not consider work complete until all four commands succeed.
