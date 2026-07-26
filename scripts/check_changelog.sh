#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# StellarTip Contract — Changelog enforcement bot
#
# Companion to Issue #112 (assignee: @precy41).
#
# Problem statement
# -----------------
# PRs that change the user-visible behaviour of the contract (e.g. add or
# remove a `pub fn` on the contract's public surface, change a `pub struct`
# field, or alter a `pub enum` variant) must include a corresponding entry
# in `CHANGELOG.md` under the `[Unreleased]` section.  Without an automated
# check, contributors routinely forget this and reviewers catch it after
# the merge.
#
# Behaviour
# ---------
# 1. Compute an effective base commit:  the merge-base of `$BASE_REF` and
#    `$HEAD_REF`.  Falling back to `$BASE_REF` keeps the script usable
#    when the local checkout is shallow / uncached.
# 2. Diff `src/lib.rs` *only* between the effective base and HEAD, looking
#    for added (`+`) or removed (`-`) lines whose first non-whitespace
#    identifier is `pub`.  This deliberately catches:
#      - top-level `pub fn / pub struct / pub enum / pub const / pub type / pub trait`
#      - contract entry points `pub fn` declared inside `#[contractimpl] impl …`
#      - `pub` fields on a struct (`pub username: Symbol,`)
#      - `pub` variants on an enum (`MyVariant = 1,`)
#      - `pub mod / pub use` declarations.
# 3. If the public-surface diff is non-empty AND `CHANGELOG.md` has NOT
#    been modified in the same range, fail with a `::error::` annotation
#    so the message surfaces inline on the GitHub PR.
# 4. If either condition is unsatisfied (no pub diff, or CHANGELOG was
#    also touched), exit 0.
#
# Environment variables
# ---------------------
#   BASE_REF   – base commit/branch to compare against.  Default:  origin/main
#   HEAD_REF   – tip commit/branch.  Default:  HEAD
#
# Usage (local)
# -------------
#   BASE_REF=origin/main HEAD_REF=HEAD bash scripts/check_changelog.sh
#
# Usage (GitHub Actions PR context)
# ---------------------------------
#   BASE_REF: ${{ github.event.pull_request.base.sha }}
#   HEAD_REF: ${{ github.sha }}
# ---------------------------------------------------------------------------

set -euo pipefail

BASE_REF="${BASE_REF:-origin/main}"
HEAD_REF="${HEAD_REF:-HEAD}"

# ---------------------------------------------------------------------------
# Resolve an effective base commit.
# ---------------------------------------------------------------------------

if ! git rev-parse --verify --quiet "${HEAD_REF}^{commit}" >/dev/null; then
  echo "::error::HEAD_REF '${HEAD_REF}' is not a valid commit."
  exit 2
fi

if git rev-parse --verify --quiet "${BASE_REF}^{commit}" >/dev/null; then
  EFFECTIVE_BASE="$(git merge-base "${BASE_REF}" "${HEAD_REF}" 2>/dev/null || echo "${BASE_REF}")"
else
  echo "::warning::BASE_REF '${BASE_REF}' not found locally; skipping changelog check."
  echo "This is expected for the very first PR before the base ref is cached."
  exit 0
fi

echo "Diffing src/lib.rs and CHANGELOG.md between"
echo "  base  = ${EFFECTIVE_BASE}"
echo "  head  = ${HEAD_REF}"
echo ""

# ---------------------------------------------------------------------------
# 1. Public-API-surface diff in src/lib.rs.
# ---------------------------------------------------------------------------

# `git diff --unified=0` removes surrounding context so we get just the
# changed lines.  `--diff-filter=AM` would be tempting but it also hides
# modifications, and a renamed signature IS a public-API change.
api_changes="$(
  git diff --unified=0 "${EFFECTIVE_BASE}" "${HEAD_REF}" -- src/lib.rs 2>/dev/null \
    | grep -E '^[+-][[:space:]]*pub[[:space:]]' \
    || true
)"

# ---------------------------------------------------------------------------
# 2. Modification check on CHANGELOG.md.
# ---------------------------------------------------------------------------

changelog_names="$(
  git diff --name-only "${EFFECTIVE_BASE}" "${HEAD_REF}" -- CHANGELOG.md 2>/dev/null || true
)"
changelog_modified=false
if [ -n "${changelog_names}" ]; then
  changelog_modified=true
fi

# ---------------------------------------------------------------------------
# 3. Enforce the contract.
# ---------------------------------------------------------------------------

if [ -n "${api_changes}" ] && [ "${changelog_modified}" = false ]; then
  {
    echo "::error::Public API in src/lib.rs changed but CHANGELOG.md was not updated."
    echo ""
    echo "Affected public items:"
    echo "${api_changes}"
    echo ""
    echo "Please add a corresponding entry under the [Unreleased] section of CHANGELOG.md."
    echo "(See docs/API_REFERENCE.md for the canonical public surface.)"
    echo "Tracking issue: StellarTip-Contract #112."
  } >&2
  exit 1
fi

if [ -n "${api_changes}" ]; then
  echo "Public API changed and CHANGELOG.md was updated ✔"
elif [ "${changelog_modified}" = true ]; then
  echo "CHANGELOG.md was updated (no public-API diff detected) ✔"
else
  echo "No public-API changes detected ✔"
fi

exit 0
