**Status:** current
**Pinned commit:** `107ee1a60a`
**Last verified:** 2026-08-21

# 04 - Build and release

## Prerequisites

Install Rust through `rustup`, then install Linux dependencies:

```sh
script/linux
```

Upstream's Linux guide is `docs/src/development/linux.md`.

## Build policy

Use `rwvk/binary-runner` on Blacksmith for the default release-grade Linux Zed binary. It builds the published `rwvk/zed` ref on a Blacksmith Ubuntu runner and retains the resulting `zed-linux-x86_64` artifact for 14 days. Select Cargo profile `release` for the optimized artifact: it enables ThinLTO and a single codegen unit, trading build time for runtime optimization.

Local builds are for development, source-level debugging, or changes that have not been published yet. Do not replace the installed WSLg binary with an unverified local build when a matching Blacksmith artifact is available.

### Blacksmith release binary

```sh
gh workflow run '96-Core Binary Runner' \
  --repo rwvk/binary-runner \
  --ref main \
  -f target_os=linux \
  -f runner=blacksmith-32vcpu-ubuntu-2404 \
  -f profile=release \
  -f ref=fork/main
```

Wait for the run to pass, download the `zed-linux-x86_64` artifact, verify that its Zed commit matches the requested ref, and atomically replace the installed editor binary. The builder checks out only published commits, so commit and push a source fix before dispatching it.

### Development build

```sh
cargo run
```

Targeted validation for the planned patch:

```sh
cargo check -p acp_thread -p agent_ui
./script/clippy -p acp_thread -p agent_ui
cargo test -p acp_thread
cargo test -p agent_ui
```

## Isolated local installation

Do not overwrite upstream `~/.local/bin/zed` while the fork is experimental.
The packaging patch must install distinct identities:

- CLI: `~/.local/bin/zed-fork`
- Editor: `~/.local/libexec/zed-fork-editor`
- Desktop id: `dev.zed.ZedFork`
- Application data/config paths that do not silently corrupt upstream state

Until that patch exists, run the fork with `cargo run` from this worktree.
Using upstream `script/install-linux` currently installs as `zed` and is not an
isolated fork release procedure.

## Paired runtime

The Zed fork still launches `opencode-fork acp` from its settings. The local
OpenCode fork must be built and installed independently. Record both build
identifiers in test reports.

## Release checklist

1. Rebase `fork/main` on reviewed `upstream/main`.
2. Run targeted checks/tests, then workspace checks appropriate to the change.
3. Run the fork-doc audit.
4. Verify isolated executable and desktop identity.
5. Verify telemetry/update settings before first launch.
6. Smoke-test a real existing and new OpenCode ACP thread.
7. Test queue, steer, cost/context updates, and nested subagent activity.
8. Tag `fork-v<upstream-version>-<sequence>` only after all gates pass.
