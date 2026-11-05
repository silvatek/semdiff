# Agent instructions

## Semdiff round trip

Before you open a pull request, and again after any later commit, check that semdiff can replay your change onto its parent.

Build semdiff from `HEAD`. The left tree is the merge-base of `HEAD` and the pull request base, `origin/main`. The right tree is `HEAD`. Apply the semdif onto a copy of the left tree. The applied tree must match the right tree.

Commit first. `git archive` reads the committed trees, so an uncommitted edit is outside the check.

```bash
git fetch origin main
cargo build
base=$(git merge-base HEAD origin/main)
rm -rf /tmp/semdiff-base /tmp/semdiff-head /tmp/semdiff-applied
mkdir -p /tmp/semdiff-base /tmp/semdiff-head
git archive "$base" | tar -x -C /tmp/semdiff-base
git archive HEAD | tar -x -C /tmp/semdiff-head
./target/debug/semdiff /tmp/semdiff-base /tmp/semdiff-head /tmp/pr.semdiff
cp -a /tmp/semdiff-base /tmp/semdiff-applied
./target/debug/semdiff --apply /tmp/pr.semdiff /tmp/semdiff-applied
diff -rq /tmp/semdiff-applied /tmp/semdiff-head
```

`diff -rq` prints nothing when the check passes. Any other result means the change or semdiff failed. Fix the branch and run the check again.

## Opting out

A change that semdiff cannot replay still has to be mergeable. That includes a fix to semdiff itself: the tool under repair cannot verify its own repair.

Skip the check when the pull request body contains this line:

```
semdiff-check: skip <why this change cannot be verified>
```

The reason is the rest of the line, and it is required. A `semdiff-check: skip` line with nothing after it does not count. Leave the line in the body so a later CI check can recognize the same opt-out.

## Scope of a change

`docs/uncaptured-changes.md` records which gaps are fixed, still to do, deferred, or left as they are. Honor the status column. A `Leave` row stays as it is. A `Fix later` row waits.

## Explanations

A new type or function carries a comment that says what it is for.

## When to stop

If you are having to guess at the user's intentions, or thinking things like "Maybe the user...", stop and ask for clarification.

If significant complexity is being revealed, stop and create a high-level, multi-step plan. Review that plan in the agent chat. Once the plan is agreed, do the low-level design and implementation for the first step only.

The objective is gradual, iterative design and implementation, with lots of small changes rather than a few huge ones.
