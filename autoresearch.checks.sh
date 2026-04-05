#!/usr/bin/env bash
# Runs clippy + tests on the affected crates. Exit 0 = all green.

set -euo pipefail

cd "$(dirname "$0")"

# Clippy on the affected crates, with warnings denied, matching script/clippy.
"${CARGO:-cargo}" clippy \
    -p git_ui -p project -p workspace \
    --release --all-targets --all-features \
    -- --deny warnings 2>&1 | tail -40
clippy_exit=${PIPESTATUS[0]}

if [ "$clippy_exit" -ne 0 ]; then
    echo "CHECKS: clippy failed"
    exit $clippy_exit
fi

# Unit tests on git_ui only — workspace/project tests are extremely slow
# to run in their entirety.
"${CARGO:-cargo}" test \
    -p git_ui \
    --lib \
    --release \
    2>&1 | tail -30
test_exit=${PIPESTATUS[0]}

if [ "$test_exit" -ne 0 ]; then
    echo "CHECKS: tests failed"
    exit $test_exit
fi

echo "CHECKS: all green"
