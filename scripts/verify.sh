#!/bin/sh

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$script_dir/.."

mode=${1:-full}

run_fast_gate() {
    cargo fmt --check
    cargo check --locked --all-targets --all-features
    cargo metadata --locked --no-deps --format-version 1 >/dev/null
    cargo clippy --locked --all-targets --all-features -- -D warnings
    JJ_CONFIG= cargo test --locked --all-targets --all-features
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'error: required command not found: %s\n' "$1" >&2
        exit 2
    fi
}

run_full_gate() {
    case $(uname -s) in
        Linux | Darwin) ;;
        *)
            printf '%s\n' 'error: the strict jj/PTY gate is supported on Linux and macOS' >&2
            exit 2
            ;;
    esac

    require_command jj
    require_command expect

    actual_rust=$(rustc --version | awk '{print $2}')
    if [ "$actual_rust" != "1.93.1" ]; then
        printf 'error: Rust 1.93.1 is required, found %s\n' "$actual_rust" >&2
        exit 2
    fi

    actual_jj=$(jj --version)
    if [ "$actual_jj" != "jj 0.44.0" ]; then
        printf 'error: jj 0.44.0 is required, found %s\n' "$actual_jj" >&2
        exit 2
    fi

    run_fast_gate

    JJ_CONFIG= \
        JJC_REQUIRE_INTEGRATION=1 \
        JJC_EXPECT_JJ_VERSION=0.44.0 \
        cargo test --locked \
        --test smoke --test tty --test diff_tree_entries --test merge_markers

    install_root=$(mktemp -d "${TMPDIR:-/tmp}/jjc-install-check.XXXXXX")
    trap 'rm -rf -- "$install_root"' EXIT HUP INT TERM
    cargo install --locked --offline --path . --root "$install_root" --force
    "$install_root/bin/jjc" --version
    JJ_CONFIG= "$install_root/bin/jjc" doctor
}

case "$mode" in
    fast)
        run_fast_gate
        ;;
    full)
        run_full_gate
        ;;
    *)
        printf 'usage: %s [fast|full]\n' "$0" >&2
        exit 2
        ;;
esac
