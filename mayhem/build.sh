#!/usr/bin/env bash
#
# boson-lang/mayhem/build.sh — build the lexer + parser fuzz targets as sanitized libFuzzer
# binaries (cargo-fuzz + ASan via RUSTFLAGS, OSS-Fuzz Rust path). The fuzzed code is the
# `boson` crate (boson::lexer::LexerAPI, boson::parser::Parser). Our additive cargo-fuzz crate
# (mayhem/fuzz/) drives the SAME two entry points as the old fork's boson/fuzz, on modern
# libfuzzer-sys; cargo-fuzz targets it via `--fuzz-dir mayhem/fuzz`.
#
# AIR-GAPPED CONTRACT (SPEC §6.5): the PATCH tier re-runs THIS script OFFLINE. The FIRST build
# (in CI/docker build, online) populates the cargo registry under $CARGO_HOME and materialises the
# third_party/syscall-numbers submodule (a boson path-dependency). The offline re-run finds both
# already on disk and does no network I/O. So do NOT hard-code `--offline` (it would break the
# first, online build); the rlenv runtime exports CARGO_NET_OFFLINE=true for the re-run.
set -euo pipefail

# clang/rustc reject SOURCE_DATE_EPOCH='' — must be unset or a valid integer.
[ -n "${SOURCE_DATE_EPOCH:-}" ] || unset SOURCE_DATE_EPOCH

: "${SRC:=/mayhem}"
: "${MAYHEM_JOBS:=$(nproc)}"
export MAYHEM_JOBS CARGO_BUILD_JOBS="$MAYHEM_JOBS"
cd "$SRC"

# --- materialise the syscall-numbers submodule (boson's `../third_party/syscall-numbers` path dep).
# It ships as a git gitlink (empty working-tree dir), so populate it ONCE, online, on the first build.
# Idempotent + air-gapped: skip entirely if it already has its Cargo.toml (the offline re-run path).
SCN_DIR="$SRC/third_party/syscall-numbers"
SCN_URL="https://github.com/Narasimha1997/syscall-numbers.git"
SCN_COMMIT="7aedc1952e38c967f48bacc69afcc8850c6e0ff8"   # pinned = the upstream submodule gitlink
if [ ! -f "$SCN_DIR/Cargo.toml" ]; then
  echo "=== materialising submodule third_party/syscall-numbers @ $SCN_COMMIT ==="
  mkdir -p "$SCN_DIR"
  git -c advice.detachedHead=false clone "$SCN_URL" "$SCN_DIR"
  git -C "$SCN_DIR" checkout -q "$SCN_COMMIT"
else
  echo "=== third_party/syscall-numbers already present — offline/idempotent re-run ==="
fi

# NOTE on sanitizers: the base exports $SANITIZER_FLAGS (clang ASan+UBSan, halting) as the C/C++
# default, but rustc IGNORES clang flags — cargo-fuzz drives Rust instrumentation via RUSTFLAGS
# (-Zsanitizer=address) instead. We reference $SANITIZER_FLAGS here only to record that the fuzzed
# code IS sanitized (the Rust way, below), not through that env var.
echo "base SANITIZER_FLAGS (informational; rustc uses RUSTFLAGS ASan instead): ${SANITIZER_FLAGS:-<unset>}"

FUZZ_DIR="mayhem/fuzz"
TRIPLE="x86_64-unknown-linux-gnu"
FUZZ_TARGETS=()
for f in "$FUZZ_DIR"/fuzz_targets/*.rs; do FUZZ_TARGETS+=("$(basename "${f%.*}")"); done
[ "${#FUZZ_TARGETS[@]}" -gt 0 ] || { echo "ERROR: no fuzz targets under $FUZZ_DIR/fuzz_targets/" >&2; exit 1; }

# OSS-Fuzz Rust libFuzzer+ASan flags. --cfg fuzzing matches libfuzzer-sys; force-frame-pointers aids
# ASan backtraces. $RUST_DEBUG_FLAGS threads the debug-info contract (DWARF < 4, §6.2 item 10): rustc
# on recent nightlies emits DWARF-5 and the precompiled ASan runtime archive lands DWARF-5 CUs FIRST,
# so -Z dwarf-version=3 alone isn't enough — the Dockerfile's cc-wrapper linker prepends a DWARF3
# anchor object so the FIRST .debug_info CU (what Mayhem's triage / verify-repo reads) is DWARF v3.
export RUST_DEBUG_FLAGS="${RUST_DEBUG_FLAGS:--C debuginfo=2 -Z dwarf-version=3 -Clinker=/opt/mayhem-dwarf3-anchor/cc-wrapper.sh}"
export RUSTFLAGS="${RUSTFLAGS:-} --cfg fuzzing -Zsanitizer=address $RUST_DEBUG_FLAGS -Cforce-frame-pointers"
echo "=== cargo fuzz build (image-default nightly, ASan via RUSTFLAGS) ==="
echo "RUSTFLAGS=$RUSTFLAGS"
echo "targets: ${FUZZ_TARGETS[*]}"

for t in "${FUZZ_TARGETS[@]}"; do
  echo "--- building fuzz target: $t ---"
  cargo fuzz build --fuzz-dir "$FUZZ_DIR" -O --debug-assertions "$t"
  bin="$SRC/$FUZZ_DIR/target/$TRIPLE/release/$t"
  [ -x "$bin" ] || { echo "ERROR: fuzz binary not found at $bin" >&2; exit 1; }
  cp "$bin" "/mayhem/$t"; echo "built /mayhem/$t"
done
echo "build.sh complete:"; ls -la /mayhem/fuzz_lexer /mayhem/fuzz_parser
