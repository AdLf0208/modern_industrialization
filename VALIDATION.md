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
| package build | `cargo horizon-mod build` (twice, `rm -f Cargo.lock` before each) | byte-identical, `e495ddae…8a14` |
| package shape | `cargo horizon-mod check modern_industrialization.hmod` | exit 0, **46,145 B**, SHA-256 `e495ddae3a5d63d81555b209a85dd5e9617267566fa3015f35397da6922f8a14`, 59 entries, no packed `Cargo.lock` |
| forward fit | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | `0b14f8a  made for` |
| forward fit (a later build) | `cargo horizon-mod fits modern_industrialization.hmod --build f00125f` | `f00125f  fits` (the 2026-11-20 repair; `notes.md` §Registry fit-gate repair) |
| package gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | **1/1** |
| in-game (modded) | `launcher-game.sh run modern_industrialization` | **PASS** + `EXIT:0` (`evidence/modern_industrialization/e2e-default-0.2.0.log`) |
| in-game (control) | the same command on `GAME=$CONTROL_GAME` | **FAIL** + `EXIT:1` (`e2e-control-nopackage.log`) |
| fail-capability | `SELFTEST=1` | **FAIL** + `EXIT:1` (`e2e-selftest-0.2.0.log`) |
| log gate | `python3 ports/tools/check-e2e-logs.py modern_industrialization --strict` | 6 ok, 0 missing-exit, 0 bad-exit |

Every step ran through `ports/tools/build-slot.sh` (`--job sdk-mi`, `BUILD_LOCAL=1`: the second
box has no SDK, no `wasm32-unknown-unknown` target and not the SDK toolchain). Logs:
`ports/_shared/evidence/modern_industrialization/sdk-*.log` and
`ports/_shared/evidence/modern_industrialization-e2e/`.

**The in-game leg runs now** (2026-11-20): launcher 0.2.78 builds a game at `0b14f8a` carrying the
package, and the scenario in `tools/e2e.sh` reaches `PASS` + `EXIT:0`. The declared container the
leg first could not see is fixed — `notes.md` §"The declared container the game could not see"
records the defect, the symbol chain and the pinning assertion.
