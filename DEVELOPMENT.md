# Development

## Repository layout

```
core/      ccl-core: gateway calls, model rules, settings files. No terminal.
  src/config.rs    ~/.ccl/config.json and its defaults
  src/keys.rs      ~/.ccl/keys.json (ccc's format), masking, validation
  src/models.rs    suffix filter, versions, filling the four slots
  src/gateway.rs   GET /v1/models, POST /v1/messages probe
  src/settings.rs  .claude/settings.local.json merge, template, .gitignore
  tests/           corpus tests: the team's model list, settings round-trips
cli/       ccl: the terminal over ccl-core
  src/picker.rs    the Switch Model screen
  src/term.rs      marks, hidden input, colour switch
  src/commands/    one file per command
scripts/   check-core-deps.sh
install.ps1, install.sh
.github/workflows/  ci.yml (lint + test on 3 OSes), release.yml (tag → binaries)
```

The dependency direction is one-way: `cli` depends on `core`, never the reverse, and `core` must not pull in `clap`, `crossterm` or `anyhow`. `scripts/check-core-deps.sh` enforces it in CI.

## Building and testing

```bash
cargo build                      # target/debug/ccl
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
bash scripts/check-core-deps.sh
```

To see the picker as it is drawn without a terminal:

```bash
cargo test -p ccl screen -- --nocapture
```

To try commands without touching your real `~/.ccl`, point `CCL_HOME` at a scratch folder:

```bash
CCL_HOME=/tmp/ccl-home target/debug/ccl config path
```

## Conventions

- Every module opens with a `//!` comment saying what it is for and the rule it keeps.
- Constants have names and a comment saying why the value is what it is.
- `core` returns `ccl_core::Error`; each variant is a different thing for the user to do. The CLI uses `anyhow` on top.
- No model id is hardcoded outside tests. Behaviour that changes with the gateway belongs in `config.json`.
- Tests read as a corpus of real cases (`core/tests/models.rs` uses the gateway's actual list).

## Releasing

1. Make sure `main` is green.
2. Tag and push:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

`release.yml` builds Windows x64, Linux x64 and macOS (arm64 + x64), bakes the tag into `ccl --version` through `CCL_VERSION`, checks it, and publishes bare binaries named `ccl-<target>`. Those names are what the install scripts and `ccl update` download, so keep all three in step if a target is added.

A release can also be started by hand from the Actions tab (`workflow_dispatch`, version without the `v`).
