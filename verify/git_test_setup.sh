#!/usr/bin/env bash
# Creates a reproducible git repo state with staged + unstaged changes
# on multiple files, as required by TASK.md's verification plan.

set -euo pipefail

REPO_DIR="${1:-/tmp/zed-verify-repo}"

rm -rf "$REPO_DIR"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"

git init -q -b main
git config user.email "verify@example.com"
git config user.name "Verify Bot"

cat > README.md <<'EOF'
# Test Repository

This repository exists only to verify the git UI changes in GitFileDiffView.

## Initial content

Line 1
Line 2
Line 3
EOF

cat > greet.py <<'EOF'
def greet(name):
    return f"Hello, {name}!"


if __name__ == "__main__":
    print(greet("world"))
EOF

cat > config.toml <<'EOF'
[server]
host = "localhost"
port = 8080
log_level = "info"
EOF

git add README.md greet.py config.toml
git commit -q -m "Initial commit"

# Add a second commit so file_history has > 1 entry — exercises the
# prev/next file navigation arrows in CommitViewToolbar (Issue 5b).
cat >> greet.py <<'EOF'


def farewell(name):
    return f"Goodbye, {name}!"
EOF
git add greet.py
git commit -q -m "Add farewell function"

# Staged + unstaged changes on README.md (partially staged file) — used
# to verify Issue 1's WorkingVsIndex vs IndexVsHead diff bases.
cat > README.md <<'EOF'
# Test Repository

This repository exists only to verify the git UI changes in GitFileDiffView.

## Staged content (v2)

Staged-1
Staged-2
Staged-3
EOF
git add README.md

cat > README.md <<'EOF'
# Test Repository

This repository exists only to verify the git UI changes in GitFileDiffView.

## Staged content (v2)

Staged-1
Unstaged-hunk
Staged-3
EOF

# Fully unstaged change on greet.py.
cat >> greet.py <<'EOF'


def shout(name):
    return greet(name).upper()
EOF

# Fully staged change on config.toml.
cat > config.toml <<'EOF'
[server]
host = "0.0.0.0"
port = 9090
log_level = "debug"
EOF
git add config.toml

echo "Repo initialised at: $REPO_DIR"
echo "--- git status ---"
git status --short
echo "--- git log ---"
git log --oneline
