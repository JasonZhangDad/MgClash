# MgClash

MgClash is a cross-platform desktop proxy client targeting:

- macOS 13+ on Intel (`x86_64`) and Apple Silicon (`aarch64`)
- Windows 10/11 on `x86_64`
- Ubuntu 22.04+ on `x86_64`

The current milestone is the cross-platform foundation. Product behavior is
defined by the original [V1.0 PRD](Magies_Proxy_PRD_V1.0.md) together with the
[V1.1 cross-platform addendum](docs/PRD_V1.1_CROSS_PLATFORM_ADDENDUM.md). When
the documents conflict, V1.1 takes precedence.

## Development

```sh
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Release artifacts are intentionally unsigned during the current development
phase. See the V1.1 addendum for the resulting platform limitations.
