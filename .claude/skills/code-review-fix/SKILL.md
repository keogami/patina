---
name: code-review-fix
description: Apply fixes selected from a prior code-review run. Reads `.claude/skills/code-review/pending-fixes.yaml` and edits the listed files. Use whenever the user says "apply the fixes", "implement the review", references a `pending-fixes.yaml`, or follows a `code-review` skill invocation that ended with a handoff line.
---

# Code review — apply fixes

Consumes the payload produced by the `code-review` skill and edits files in place.

## Preflight

```bash
test -f .claude/skills/code-review/pending-fixes.yaml || echo "no pending fixes"
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || echo "not a git repo"
```

If the payload is missing, tell the user to run `code-review` first and stop.

## Staleness check

The payload includes `source_diff_ref`. Compare the current diff against it:

```bash
git diff <source_diff_ref> --stat
```

If files in the payload have changed since the review was generated, line numbers may be wrong. List the affected findings and ask the user whether to (a) proceed anyway with best-effort relocation, (b) skip the stale ones, or (c) abort and re-run `code-review`.

## Apply loop

For each finding in order:

1. Read the target file around the listed lines (read ~10 lines of surrounding context — never edit blind).
2. Verify the `problem` description still matches what's actually there. If it doesn't (the code has changed, or the line range is off), search the file for the pattern described in `problem`. If still not found, mark the finding as skipped with a one-line reason and move on.
3. Apply the change described in `fix`. The `fix` field is intentionally prose — interpret it the way a competent engineer would, not as a literal patch.
4. Print a short confirmation: `[F<id>] applied: <one line summary of what changed>` or `[F<id>] skipped: <reason>`.

Do not run formatters, linters, or tests. The user has their own loop for that.

## Atomicity

Apply all selected fixes in one pass, then summarize at the end:

```
Applied N, skipped M.
Skipped: [F2] line range no longer matched; [F5] file deleted since review.
```

Do not commit, do not `git add`. Read-write on files only.

## Cleanup

After the summary, move the payload aside so it can't be re-applied accidentally:

```bash
mv .claude/skills/code-review/pending-fixes.yaml \
   .claude/skills/code-review/applied-$(date -u +%Y%m%dT%H%M%SZ).yaml
```

Keeps an audit trail; the next `code-review` run writes a fresh `pending-fixes.yaml`.

## What not to do

- Do not re-do the review. The `fix` field is the spec; trust it.
- Do not apply fixes that are no longer needed (problem already gone).
- Do not commit or stage anything.
- Do not delete the payload — archive it.
