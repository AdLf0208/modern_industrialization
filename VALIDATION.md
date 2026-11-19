# Validation — the SDK ladder (2026-11-18, in-game leg added 2026-11-20)

The `.hnmod` ladder this file used to describe (staged `horizon.py check`/`test`/`build`, the
shared `horizon-ctl`, `SELFTEST=1`) applied to pin `1d63e0a5` and is kept in `notes.md` as the
behaviour contract of slice 1. What this port is validated with now is the SDK's own ladder, at
nightly `0b14f8acfd97fa2c750e42795249dcc981178ca4` / mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`:

| Step | Command | Result |
|---|---|---|
| pure core | `cargo test --offline` | **27 passed; 0 failed** |
| lints | `cargo clippy --offline --all-targets -- -D warnings` | exit 0, 0 warnings |
| format | `rustfmt --edition 2024 --check src/*.rs` | clean |
| package build | `cargo horizon-mod build` (twice, `rm -f Cargo.lock` before each) | byte-identical, `db21a20b…4873` |
| package shape | `cargo horizon-mod check modern_industrialization.hmod` | exit 0, **46,651 B**, SHA-256 `db21a20b6c16790ac097831a9d936b803d2c69849b357f7aa2d2105031dc4873`, 59 entries, no packed `Cargo.lock` |
| forward fit | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | `0b14f8a  made for` |
| package gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | **1/1** |
| in-game (modded) | `launcher-game.sh build modern_industrialization`, then `tools/e2e.sh` | **PASS** + `EXIT:0` (`evidence/modern_industrialization-e2e/e2e-launcher.log`) |
| in-game (control) | the same command on `GAME=$CONTROL_GAME` | **FAIL** + `EXIT:1` (`e2e-launcher-control.log`) |
| fail-capability | `SELFTEST=1` | **FAIL** + `EXIT:1` (`e2e-launcher-selftest.log`) |
| log gate | `python3 ports/tools/check-e2e-logs.py modern_industrialization-e2e --strict` | 4 ok, 0 missing-exit, 0 bad-exit |

Every step ran through `ports/tools/build-slot.sh` (`--job sdk-mi`, `BUILD_LOCAL=1`: the second
box has no SDK, no `wasm32-unknown-unknown` target and not the SDK toolchain). Logs:
`ports/_shared/evidence/modern_industrialization/sdk-*.log` and
`ports/_shared/evidence/modern_industrialization-e2e/`.

**The in-game leg runs now** (2026-11-20): launcher 0.2.78 builds a game at `0b14f8a` carrying the
package, and the scenario in `tools/e2e.sh` reaches `PASS` + `EXIT:0`. The declared container the
leg first could not see is fixed — `notes.md` §"The declared container the game could not see"
records the defect, the symbol chain and the pinning assertion.
