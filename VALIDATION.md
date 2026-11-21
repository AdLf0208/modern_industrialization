# Validation — the SDK ladder (slice 2 2026-11-21; hatches slice 3 2026-11-21)

The `.hnmod` ladder this file used to describe (staged `horizon.py check`/`test`/`build`, the
shared `horizon-ctl`, `SELFTEST=1`) applied to pin `1d63e0a5` and is kept in `notes.md` as the
behaviour contract of slice 1. What this port is validated with now is the SDK's own ladder at the
campaign pin, nightly `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` / mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, package **0.2.2**:

| Step | Command | Result |
|---|---|---|
| pure core | `cargo test --offline` | **50 passed; 0 failed** (27 from slice 1 + 14 from slice 2 + 9 from slice 3) |
| lints | `cargo clippy --offline --all-targets -- -D warnings` | exit 0, 0 warnings |
| format | `cargo fmt --check` | clean |
| package build | `cargo horizon-mod build` (twice, `rm -f Cargo.lock` before each) | byte-identical, `d2d388ce…` |
| package shape | `cargo horizon-mod check modern_industrialization.hmod` | exit 0, **103,597 B**, SHA-256 `d2d388ce1932fbd94afeb75c2d804951692f992d0786c0596e41fddf22276ae7`, **129 entries**, no packed `Cargo.lock` |
| forward fit | `cargo horizon-mod fits modern_industrialization.hmod --build 0a875da` | `0a875da  made for` |
| forward fit (a later build) | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | `0b14f8a  fits` |
| package gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | **1/1** |
| in-game (modded) | `launcher-game.sh build modern_industrialization` (`native-0a875da-7f4050165798`), then `tools/e2e.sh` | **PASS** + `EXIT:0` (`evidence/modern_industrialization-e2e/e2e-launcher.log`) |
| in-game (control) | the same command on `GAME=$CONTROL_GAME` | **FAIL** + `EXIT:1` (`e2e-launcher-control.log`) |
| fail-capability | `SELFTEST=1` (inverts the coke oven's unformed counterfactual) | **FAIL** + `EXIT:1` (`e2e-launcher-selftest.log`) |
| log gate | `python3 ports/tools/check-e2e-logs.py modern_industrialization-e2e --strict` | logs=5 ok=5, 0 missing-exit, 0 bad-exit |

Every step ran through `ports/tools/build-slot.sh` (`--job W1-modern_industrialization`,
`BUILD_LOCAL=1`: the second box has no SDK, no `wasm32-unknown-unknown` target and not the SDK
toolchain). `CARGO_INCREMENTAL=0`, no `--target-dir`, no private slot directory. Logs:
`ports/_shared/evidence/modern_industrialization/sdk-*.log` (each names the sha256 it ran against)
and `ports/_shared/evidence/modern_industrialization-e2e/`. The e2e honours `E2E_TMP` (the launcher
exports `~/.cache/horizon-e2e/scratch`, so its scratch world never lands on the `/tmp` tmpfs).

**Slice 3's red proof** is `evidence/modern_industrialization/sdk-test-prelog.log`: the new tests and
the new shell API existed with the glue still on the old signature, which is
`error[E0053]: method 'block_at' has an incompatible type for trait` +
`error[E0061]: this function takes 3 arguments but 2 arguments were supplied`, exit 101. The first
run of the new tests then failed `the_three_by_three_by_four_add_on_shell_matches` on a fixture
offset that was a brick rather than a coil; the corrected fixture pins 17 bricks + 16 coil positions.

**The in-game leg is fail-capable** (2026-11-21): the launcher builds a game at `0a875da` carrying
the package; the scenario in `tools/e2e.sh` builds the coke oven's 3×3×3 shell with three bronze
hatches, runs both jar recipes **through the item input hatch** and reads the product out of the
output hatch for exactly 12,000 FE each, breaks one brick (player break) to freeze a run and reads
`formed false, missing 1` for 120 ticks, refuses a coil in the gap, resumes on the brick, refuses a
hatch in a middle-ring position, reads the coke oven recipe kind back with
`/modern_industrialization recipes`, and matches the add-on's 3×3×4 shell (17 bronze-plated bricks +
16 coil positions, two hatches) through `/modern_industrialization shape`; slice 1's and slice 2's
legs are the regression. The control is the same Nightly without the package (`EXIT:1`), and
`SELFTEST=1` inverts the unformed-oven assertion (`EXIT:1`).
