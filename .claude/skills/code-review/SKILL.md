---
name: code-review
description: Review the user's current uncommitted code changes using difftastic for structural diffing. Checks three things — idiomatic usage for the language, duplicated logic that could reuse existing code, and overcomplicated solutions for simple problems. Use whenever the user asks to review, critique, audit, sanity-check, or "look over" their uncommitted/staged/unstaged work, their working tree, what they're "about to commit", their current changes, their WIP, or any phrasing that implies inspecting in-progress git changes before commit. Trigger even when the user does not say "difftastic" or "difft" — this skill is the default code-review path when there are uncommitted changes.
---

# Code Review (uncommitted changes, difftastic-powered)

Review the user's uncommitted git changes through three lenses: **idiomatic**, **duplicative**, **overcomplicated**. Use `difft` for the diff so structural changes are clear and formatting noise is gone.

## Why difftastic
Standard `git diff` is line-oriented and shows reformatting as change. difft parses both sides into syntax trees and only reports semantic changes. This matters because two of the three checks (duplication, overcomplication) need a clean view of *what was actually added*, not what shifted on a page.

## Preflight

Before reviewing, confirm the environment. If any of these fail, stop and report — do not invent a fallback.

```bash
command -v difft >/dev/null || echo "difft not installed"
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || echo "not a git repo"
```

If the user is in a repo without difft, tell them to install it (`cargo install difftastic` or their package manager) rather than falling back to plain diff — the skill's quality depends on structural diffing.

## Scope: what counts as "uncommitted"

Default to **worktree vs HEAD** — i.e. everything they would commit if they ran `git add -A && git commit`. This is `git diff HEAD`.

```bash
git diff HEAD --name-only        # files changed
git diff HEAD --name-only --diff-filter=A   # newly added
```

If the user explicitly says "staged" or "what I've added", switch to `git diff --cached`. If they say "unstaged", use `git diff` (no ref).

Skip binary files, lockfiles (`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `poetry.lock`, `go.sum`), and generated files. Mention what was skipped so the user can override.

## Getting the diff

For each changed text file, get the structural diff:

```bash
GIT_EXTERNAL_DIFF=difft DFT_DISPLAY=inline git diff HEAD -- <file>
```

`GIT_EXTERNAL_DIFF` is honored directly by `git diff` — no `--ext-diff` flag needed (that's only for `git log`/`git show`). `DFT_DISPLAY=inline` avoids the two-column default that wastes horizontal space.

For files where difft falls back to line-oriented diff (parse errors, unsupported language), it says so in its output. Note it and proceed.

Also fetch the **current file contents** for context — diffs alone don't show surrounding code that informs idiom and duplication checks:

```bash
cat <file>
```

For newly-added files, the diff alone is the full content.

## The three checks

For each file, write findings under three headings. Omit a heading if there are no findings under it. Do not pad — silence is fine.

### 1. Idiomatic

Language-specific. Look at the diff and ask: would a senior practitioner of this language write it this way? Concrete patterns to flag:

- **Rust**: `.unwrap()` in non-test/non-main code, `match` where `if let` suffices, manual loops where iterators apply, `String` params that should be `&str`, missing `?` propagation, `Vec<T>` returns when `impl Iterator<Item=T>` works, premature `Arc<Mutex<T>>`.
- **Python**: explicit indexed `for i in range(len(x))` instead of `enumerate`/`zip`, building lists with `.append` in a loop instead of comprehension, `dict.keys()` in `in` checks, mutable default args.
- **Go**: ignoring errors, missing `defer` for cleanup, naked returns in long funcs, `interface{}` where a concrete type fits.
- **TypeScript**: `any` leaking through, `Promise` chains where `async/await` reads cleaner, optional chaining missed.

Don't enumerate the language — adapt to whatever languages are in the diff. The point is: identify the specific idiom violation and name the idiomatic replacement.

### 2. Duplicated logic

This is the check most reviewers skip and most worth doing. New code often re-implements something the codebase already has.

Do **not** grep the codebase yourself in the main context — search results bloat the review context with files you'll glance at once and never reference again. Delegate to the **Explore** subagent (read-only, isolated context, runs on Haiku — fast and cheap). It returns only the summary.

**Batch into a single Task call.** Sending whole code blocks to Explore burns tokens; sending one-line behavioral summaries doesn't. Workflow:

1. Walk the diff and for every non-trivial added function, method, or block, write a one-line description of what it *does* (not what it's called). Focus on inputs, outputs, and the operation. Examples:
   - *"Reads a TOML file, applies env-var overrides for keys prefixed FOO_, returns Config."*
   - *"Retries an HTTP call up to N times with exponential backoff."*
   - *"Formats a byte count as a human-readable string (e.g. 1.2 MB)."*

2. Send all summaries in **one** Explore call:

```
Task(subagent_type="Explore", thoroughness="medium", prompt="""
A code review is in progress. The user just added several pieces of logic. For each item below, find existing code in this repository that does the same or substantially overlapping work.

Items:
1. <one-line description>
2. <one-line description>
3. <one-line description>
...

For each item return:
- the matching function/item name and absolute path:line
- one sentence on how it overlaps the described behavior
- "no match" if nothing genuinely overlaps (don't stretch — superficial keyword matches don't count)
""")
```

3. When Explore returns, for each reported match, read the matched code yourself and compare against the new code. Only flag when the existing code genuinely covers the new code's intent.

Report as: *"`new_thing` at `path:line` duplicates `existing_thing` at `path:line` — consider reusing."* If unsure after comparing, phrase as a question: *"Is `existing_thing` close enough to reuse?"*

False positives are worse than misses here. Explore can be optimistic on overlap — your job in the main context is to verify.

### 3. Overcomplicated

Ask: given the apparent intent, is this the simplest solution? Common shapes of overcomplication:

- **Trait/interface with one impl** added for a change that doesn't anticipate a second impl
- **Builder pattern** for a struct with 2-3 fields
- **Generic over `T`** where one concrete type would do
- **New abstraction layer** (wrapper struct, adapter) when the underlying call sites are few
- **Async** where sync suffices and no caller awaits concurrently
- **Reinventing a popular crate.** Hand-rolling something the ecosystem already solved. Examples to scan for:
    - Rust: byte-size formatting (`humansize`, `bytesize`, `human-bytes`), duration formatting (`humantime`), retry/backoff (`backoff`, `tokio-retry`), chunk/window/group-by (`itertools`), error enums (`thiserror`), argparse (`clap`), date math (`chrono`, `jiff`), URL parsing (`url`), base64/hex (`base64`, `hex`), once-cell semantics (`std::sync::OnceLock`), progress bars (`indicatif`).
    - Python: HTTP (`requests`/`httpx` vs raw `urllib`), `pathlib` vs `os.path` string manipulation, `dataclasses`/`pydantic` vs hand-rolled `__init__`, `itertools` for batching/windowing.
    - TS/JS: `date-fns`/`dayjs` vs manual date math, `zod` vs hand-rolled validators, native `URL`/`URLSearchParams` vs string parsing.
  Only flag if the crate/module is already in the dep tree, or if pulling it in is clearly a net win (well-maintained, narrow scope, replaces real volume). Don't recommend adding a dependency for ten lines of trivial code.
- **Manual implementation** of something in stdlib (e.g. re-implementing `Iterator::group_by`, `slice::windows`)
- **Defensive code** for conditions that the type system or earlier checks already rule out

When flagging, sketch the simpler alternative in one or two lines. Vague "this could be simpler" is not useful — show what simpler looks like.

Important: distinguish *premature* complexity from *load-bearing* complexity. If the user's code is in a hot path, a perf-critical library, or a public API, complexity that looks like overkill may be intentional. Ask, don't assert.

## Output format

Every finding gets a stable ID (`F1`, `F2`, ...) numbered sequentially across the whole review. The downstream fix skill consumes these IDs and the per-finding metadata, so every finding must include:

- the ID
- `file:line` (or line range)
- category: `idiomatic` / `duplicated` / `overcomplicated`
- the problem in one sentence
- the suggested fix, concrete enough that another LLM with no review context could implement it

Template:

```markdown
# Code review: <N> files changed, <M> findings

## src/foo.rs

- [F1] L42 (idiomatic) — `.unwrap()` on Result. **Fix:** replace with `?`; enclosing fn already returns `Result`.
- [F2] L58 (idiomatic) — manual loop building `Vec<_>`. **Fix:** replace the loop with `.collect::<Vec<_>>()` on the existing `.map(...)` chain at L55.
- [F3] L100–120 (overcomplicated) — `trait ConfigSource` has one impl (`FileSource`). **Fix:** delete the trait, inline its two methods on `FileSource`.

## src/bar.py

- [F4] L12 (idiomatic) — `for i in range(len(items))`. **Fix:** use `enumerate(items)` if the index is needed, else iterate `items` directly.

---

**Highest-leverage fix:** F3 — deleting the one-impl trait removes ~20 lines and a layer of indirection.
```

End with the highest-leverage call-out. The user came for actionable signal, not a list.

## Questionnaire phase

After printing the findings, ask **one** question — not per-item prompts:

> Which findings should I fix? Reply with IDs (e.g. `F1,F3`), `all`, `none`, or a filter (e.g. `all idiomatic`, `skip duplicated`).

Then wait. Don't push past it; don't pre-emit a payload.

Parse the user's reply:
- `all` → every ID
- `none` → end cleanly, no payload, no handoff
- comma-separated IDs → those exact IDs
- category filter (`all idiomatic`, `skip overcomplicated`, etc.) → expand to matching IDs
- mixed (`F1, all overcomplicated`) → take the union
- ambiguous → echo your interpretation and confirm before proceeding

## Handoff to fix skill

If the user selected at least one finding, write the selection as a YAML payload to a stable path the fix skill can read:

**Path:** `.claude/skills/code-review/pending-fixes.yaml` (relative to repo root)

**Shape:**

```yaml
generated_at: 2026-05-20T14:30:00Z
source_diff_ref: HEAD          # the ref the diff was taken against
findings:
  - id: F1
    path: src/foo.rs
    lines: [42, 42]
    category: idiomatic
    problem: ".unwrap() on Result"
    fix: "replace with `?`; enclosing fn already returns Result"
  - id: F3
    path: src/foo.rs
    lines: [100, 120]
    category: overcomplicated
    problem: "trait ConfigSource has one impl (FileSource)"
    fix: "delete the trait, inline its two methods on FileSource"
```

Always use a `[start, end]` array for `lines` even for single-line findings (`[42, 42]`) — keeps the fix skill's parser uniform.

After writing the file, say:

> Wrote N selected fixes to `.claude/skills/code-review/pending-fixes.yaml`. Invoke the `code-review-fix` skill to apply them.

This sentence is the handoff cue. Do **not** apply fixes yourself in this skill's context — fixes belong to the dedicated skill so the review/fix concerns stay separated and the payload is auditable on disk. If the user said `none`, just confirm and end without writing the file.

## Edge cases

- **No uncommitted changes**: say so, suggest `git diff <commit>` style if they meant a different range, stop.
- **Only formatting changes**: difft will show an empty diff (or near-empty). Tell the user that's all you found and move on.
- **Very large diff (>20 files or >2000 lines)**: ask the user to narrow scope — by path, by category, or by reviewing staged-only — rather than producing a shallow review of everything.
- **Mixed-language diff**: review per file; don't try to find cross-language duplication.
- **Generated files**: skip and say so. Heuristics: header comments saying "DO NOT EDIT", `_pb2.py`, `*.generated.*`, anything in `target/`, `dist/`, `node_modules/`, `build/`.

## What not to do

- Do not nitpick style that a formatter would handle (`rustfmt`, `prettier`, `black`). The user has tools for that.
- Do not flag *changes to* idiomatic code as non-idiomatic just because the diff is small — judge the resulting code, not the delta.
- Do not invent issues to fill space. An empty section is a stronger signal than a weak finding.
- Do not run `git commit`, `git add`, or any mutating git command. Read-only.
