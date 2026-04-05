#!/usr/bin/env bash
# Adds a deeper commit history to the test repo so the file-history
# prev/next arrows have something to walk through.
set -e

REPO="${1:-/tmp/zed-verify-repo}"
cd "$REPO"

# Undo the current staged + unstaged changes first so we can commit cleanly.
git -c user.email=verify@example.com -c user.name="Verify Bot" stash -u -q || true
git reset --hard HEAD -q

# Commit 3: polish greet.py docstrings
cat > greet.py <<'EOF'
def greet(name):
    """Return a friendly greeting."""
    return f"Hello, {name}!"


def farewell(name):
    """Return a goodbye."""
    return f"Goodbye, {name}!"


if __name__ == "__main__":
    print(greet("world"))
EOF
git -c user.email=verify@example.com -c user.name="Verify Bot" \
    commit -q -am "Add docstrings to greet/farewell"

# Commit 4: add shout()
cat > greet.py <<'EOF'
def greet(name):
    """Return a friendly greeting."""
    return f"Hello, {name}!"


def farewell(name):
    """Return a goodbye."""
    return f"Goodbye, {name}!"


def shout(name):
    """Return an uppercase greeting."""
    return greet(name).upper()


if __name__ == "__main__":
    print(greet("world"))
    print(shout("world"))
EOF
git -c user.email=verify@example.com -c user.name="Verify Bot" \
    commit -q -am "Add shout() helper"

# Commit 5: add whisper()
cat >> greet.py <<'EOF'


def whisper(name):
    """Return a lowercase greeting."""
    return greet(name).lower()
EOF
git -c user.email=verify@example.com -c user.name="Verify Bot" \
    commit -q -am "Add whisper() helper"

# Commit 6: type hints
cat > greet.py <<'EOF'
def greet(name: str) -> str:
    """Return a friendly greeting."""
    return f"Hello, {name}!"


def farewell(name: str) -> str:
    """Return a goodbye."""
    return f"Goodbye, {name}!"


def shout(name: str) -> str:
    """Return an uppercase greeting."""
    return greet(name).upper()


def whisper(name: str) -> str:
    """Return a lowercase greeting."""
    return greet(name).lower()


if __name__ == "__main__":
    print(greet("world"))
    print(shout("world"))
EOF
git -c user.email=verify@example.com -c user.name="Verify Bot" \
    commit -q -am "Add type hints to greet.py"

echo "--- git log ---"
git log --oneline
