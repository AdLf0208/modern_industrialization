# Validation — the SDK ladder (2026-11-18; in-game leg 2026-11-20; slice 2 2026-11-21)

The `.hnmod` ladder this file used to describe (staged `horizon.py check`/`test`/`build`, the
shared `horizon-ctl`, `SELFTEST=1`) applied to pin `1d63e0a5` and is kept in `notes.md` as the
behaviour contract of slice 1. What this port is validated with now is the SDK's own ladder at the
campaign pin, nightly `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` / mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0` (`0b14f8a` was the pin before slice 2; the package still
fits it, which is the second `fits` line below):

| Step | Command | Result |
|---|---|---|
| pure core | `cargo test --offline` | **41 passed; 0 failed** (27 from slice 1 + 14 from slice 2) |
| lints | `cargo clippy --offline --all-targets -- -D warnings` | exit 0, 0 warnings |
| format | `rustfmt --edition 2024 --check src/lib.rs src/runtime.rs` | clean |
| package build | `cargo horizon-mod build` (twice, `rm -f Cargo.lock` before each) | byte-identical, `7308bcf2…` |
| package shape | `cargo horizon-mod check modern_industrialization.hmod` | exit 0, **62,560 B**, SHA-256 `7308bcf29c29cabca7def755bb99837a8617ffdb3813c10d4e658fd22dcd5289`, **75 entries**, no packed `Cargo.lock` |
| forward fit | `cargo horizon-mod fits modern_industrialization.hmod --build 0a875da` | `0a875da  made for` |
| forward fit (a later build) | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | `0b14f8a  fits` |
| package gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | **1/1** |
| in-game (modded) | `launcher-game.sh build modern_industrialization` (`native-0a875da-60718813c863`), then `tools/e2e.sh` | **PASS** + `EXIT:0` (`evidence/modern_industrialization-e2e/e2e-launcher.log`) |
| in-game (control) | the same command on `GAME=$CONTROL_GAME` | **FAIL** + `EXIT:1` (`e2e-launcher-control.log`) |
| fail-capability | `SELFTEST=1` (inverts the coke oven's formed counterfactual) | **FAIL** + `EXIT:1` (`e2e-launcher-selftest.log`) |
| log gate | `python3 ports/tools/check-e2e-logs.py modern_industrialization-e2e --strict` | logs=4 ok=4, 0 missing-exit, 0 bad-exit |

Every step ran through `ports/tools/build-slot.sh` (`--job W1-modern_industrialization`,
`BUILD_LOCAL=1`: the second box has no SDK, no `wasm32-unknown-unknown` target and not the SDK
toolchain). `CARGO_INCREMENTAL=0`, no `--target-dir`, no private slot directory. Logs:
`ports/_shared/evidence/modern_industrialization/sdk-*.log` (each names the sha256 it ran against)
and `ports/_shared/evidence/modern_industrialization-e2e/`.

**Slice 2's red proof** is `evidence/modern_industrialization/sdk-test-red-slice2.log`: the same
`cargo test` with the 14 new tests and no implementation, which is
`error: could not compile 'modern_industrialization' (lib test) due to 37 previous errors`
(exit 101) — `cannot find value 'COKE_OVEN_SHELL'`, `cannot find trait 'Shell'`,
`cannot find struct, variant or union type 'CokeOven'`.

**The in-game leg runs and is fail-capable** (2026-11-21): the launcher builds a game at `0a875da`
carrying the package, the scenario in `tools/e2e.sh` builds a 3×3×3 shell of 25 bricks, forms the
controller, runs both of the jar's coke oven recipes for exactly 12,000 FE each, breaks one brick by
a **player break** and reads `formed false, missing 1` with the run frozen for 120 ticks, refuses to
re-form with a macerator in the gap, and finishes the paused run when the brick returns — plus
slice 1's macerator legs as the regression. The control is the same Nightly without the package
(`EXIT:1`), and `SELFTEST=1` inverts the unformed-oven assertion (`EXIT:1`).
