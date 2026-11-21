# Validation — the SDK ladder (slice 4 2026-11-21; slices 2–3 the same ladder)

The `.hnmod` ladder this file used to describe (staged `horizon.py check`/`test`/`build`, the
shared `horizon-ctl`, `SELFTEST=1`) applied to pin `1d63e0a5` and is kept in `notes.md` as the
behaviour contract of slice 1. What this port is validated with now is the SDK's own ladder at the
campaign pin, nightly `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` / mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, package **0.2.3**:

| Step | Command | Result |
|---|---|---|
| pure core | `cargo test --offline` | **59 passed; 0 failed** (27 slice 1 + 14 slice 2 + 9 slice 3 + 9 slice 4) |
| lints | `cargo clippy --offline --all-targets -- -D warnings` | exit 0, 0 warnings |
| format | `cargo fmt --check` | clean |
| package build | `cargo horizon-mod build` (twice, `rm -f Cargo.lock` before each) | byte-identical (`cmp`), no packed `Cargo.lock` |
| package shape | `cargo horizon-mod check modern_industrialization.hmod` | exit 0, **139,293 B**, SHA-256 `f665aa0bf7cb298fea4a2a730aaee4ae97837815a68ee2764a0d5264fa2c8691`, **192 entries**, `0.2.3 (native-mod, format native-1/0a875da…/e59da909…, for 26.3)` |
| forward fit | `cargo horizon-mod fits modern_industrialization.hmod --build 0a875da` | `0a875da  made for`, exit 0 |
| forward fit (a later build) | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | `0b14f8a  fits`, exit 0 |
| package gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | **1/1** |
| in-game (modded) | `launcher-game.sh build modern_industrialization` (`native-0a875da-463a09e88e81`, `Compiling modern_industrialization (1 of 1)`), then `tools/e2e.sh` | **PASS** + `EXIT:0` (`evidence/modern_industrialization-e2e/e2e-launcher-slice4.log`) |
| in-game (control) | the same command on `GAME=$CONTROL_GAME` | **FAIL** + `EXIT:1` (`e2e-launcher-slice4-control.log`) |
| fail-capability (slice 2's) | `SELFTEST=1` (inverts the coke oven's unformed counterfactual) | **FAIL** + `EXIT:1` (`e2e-launcher-slice4-selftest.log`) |
| fail-capability (slice 4's) | `NEGATIVE=1` (inverts the 11,980 FE stall: the short run must be seen to make its coke) | **FAIL** + `EXIT:1` (`e2e-launcher-slice4-negative.log`) |
| log gate | `python3 ports/tools/check-e2e-logs.py modern_industrialization-e2e --strict` | logs=10 ok=10, 0 missing-exit, 0 bad-exit |

Every step ran through `ports/tools/build-slot.sh` (`--job W1-modern_industrialization`,
`BUILD_LOCAL=1`: the second box has no SDK, no `wasm32-unknown-unknown` target and not the SDK
toolchain). `CARGO_INCREMENTAL=0`, no `--target-dir`, no private slot directory. Logs:
`ports/_shared/evidence/modern_industrialization/sdk-*.log` (each names the sha256 it ran against)
and `ports/_shared/evidence/modern_industrialization-e2e/`. The e2e honours `E2E_TMP` (the launcher
exports `~/.cache/horizon-e2e/scratch`, so its scratch world never lands on the `/tmp` tmpfs).

**Slice 4's red proofs** are `evidence/modern_industrialization/sdk-test-red-slice4-assert.log`
(the family's expectation alone, `left: 4 / right: 14`, `49 passed; 1 failed`) and
`sdk-test-red-slice4.log` (the nine new tests with nothing implemented: **36 errors**, exit 101,
e.g. `error[E0433]: cannot find type 'HatchTier' in this scope`,
`error[E0599]: no variant named 'EnergyHatch' found for enum 'runtime::HatchKind'`).

**The in-game leg is fail-capable** (2026-11-21): the launcher builds a game at `0a875da` carrying
the package; the scenario in `tools/e2e.sh` keeps slices 1–3 (the macerator's buffer, recipes,
overclock and save/relaunch; the coke oven's 3×3×3 shell, its two jar recipes through the bronze
hatches, break/freeze/resume, the middle-ring refusal, the recipe kind and the add-on's 3×3×4
matcher) and adds the energy hatch family: five tiers each with its own buffer and a clamping
over-fill, an LV energy input hatch holding a shell position while the middle ring refuses one,
exactly the 5,000 FE put into a hatch arriving in the controller's buffer, a whole 600-tick coke
run paid by exactly 12,000 FE out of the hatch against an 11,980 FE fill that stalls 2 EU short
(then 20 more FE finish it), an energy output hatch draining the controller, and the add-on's 3×3×4
shell taking an HV energy hatch with its readout naming it. The control is the same Nightly without
the package (`EXIT:1`); `SELFTEST=1` inverts the unformed-oven assertion and `NEGATIVE=1` inverts
the 11,980 FE stall (`EXIT:1` each).
