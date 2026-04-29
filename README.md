# Neon

Minimal Rust + Lua agent harness.

Install the latest binary release and start onboarding:

```bash
curl -fsSL https://raw.githubusercontent.com/arm64be/neon/main/install.sh | bash
```

Start the first-time setup flow:

```bash
NEON_CONFIG_ROOT=configs/onboarding cargo run
```

Use the ready-to-build preset directly:

```bash
NEON_CONFIG_ROOT=configs/preset cargo run
```

Starter configs are intentionally small Lua modules under `configs/onboarding`, `configs/preset`, and `configs/themes`.

Start with the documentation index:

- [Docs Index](docs/INDEX.md)
- [Installation](docs/INSTALLATION.md)
- [API Reference](docs/api/INDEX.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
