#!/bin/bash
# Pre-commit review script - MUST pass before any commit
# Usage: ./scripts/pre-commit-review.sh

set -e

echo "🔍 Pre-commit Review Starting..."

# 1. Compile check
echo "📦 Checking compilation..."
cargo check 2>&1 || { echo "❌ Compilation failed"; exit 1; }

# 2. Run all tests
echo "🧪 Running tests..."
cargo test 2>&1 || { echo "❌ Tests failed"; exit 1; }

# 3. Check for unsafe code in critical paths
echo "🔒 Security scan..."
UNSAFE_COUNT=$(grep -r "unsafe" --include="*.rs" src/executor src/client 2>/dev/null | wc -l)
if [ "$UNSAFE_COUNT" -gt 0 ]; then
    echo "⚠️  Found $UNSAFE_COUNT unsafe blocks in critical paths - review required:"
    grep -rn "unsafe" --include="*.rs" src/executor src/client 2>/dev/null || true
fi

# 4. Check for hardcoded secrets
echo "🔑 Checking for secrets..."
if grep -rE "(api_key|private_key|secret|password)\s*=\s*\"[^\"]+\"" --include="*.rs" src/ 2>/dev/null | grep -v "test" | grep -v "example" | grep -v "env::var"; then
    echo "❌ Potential hardcoded secrets found!"
    exit 1
fi

# 5. Lint check (critical lints only, allow dead_code and unused warnings)
echo "📝 Linting..."
cargo clippy --all-targets -- -A dead_code -A unused 2>&1 | grep -E "^error\[E" && { echo "❌ Clippy errors"; exit 1; } || true

# 6. Check commit message format (if provided)
if [ -n "$1" ]; then
    if ! echo "$1" | grep -qE "^(feat|fix|refactor|test|docs|chore): "; then
        echo "❌ Commit message must start with: feat|fix|refactor|test|docs|chore:"
        exit 1
    fi
    if echo "$1" | grep -qE "[一-龥]"; then
        echo "❌ Commit message must be in English!"
        exit 1
    fi
fi

echo "✅ Pre-commit review passed!"
