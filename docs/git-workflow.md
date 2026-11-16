# Git workflow and integration governance

The canonical repository is
`siebengestirn/engineering-challenge-template`. The ZIP exports and the
secondary `voluntas-market-data-next` working copy are snapshots, never
sources of Git history.

## Model

`main` represents the latest verified, integrated project. A feature branch
represents one active, bounded engineering objective. A pull request records
the evidence and decision to integrate that objective into project history.

We use protected `main` plus short-lived feature branches, mandatory pull
requests, and merge commits. There is no permanent `develop` branch, no
release branches, and no permanent feature branches. This is a single-developer
repository: a complicated GitFlow would add synchronization work without
adding an active integration team.

The machine-readable policy is [`git-policy.json`](../git-policy.json). This
document explains it; the two must not disagree.

## Rules

| Rule | Required behavior |
|---|---|
| Direct commits to `main` | Prohibited |
| Active feature branches | Temporary, one coherent deliverable each |
| Merge method | Merge commit only |
| Squash / rebase merging | Prohibited |
| Published history rewriting | Prohibited |
| Force push | Prohibited |
| CI | Required on the exact PR head before merge |
| Strict base freshness | Feature branch must be up to date with `main` when CI requires it |
| Merge review | Explicit self-review checklist plus CI evidence |
| Branch cleanup | After the merge is verified on `main` |
| Final local branches | `main` only |
| Final remote tracking | `origin/main` only |
| Stashes / worktrees | Zero stashes; one worktree unless explicitly justified |

Branch names use `feat/`, `fix/`, `docs/`, and `test/` prefixes with lowercase
topic names. Do not create `dev`, `new-main`, `main-final`, `backup-main`, or
`latest-working`.

## Branch lifecycle

```
CREATED -> ACTIVE -> PULL REQUEST -> VERIFIED MERGE -> DELETED
```

Deletion uses `git branch -d` only after the commit is an ancestor of `main`.
Remote deletion uses `git push origin --delete <branch>` only after the merge
and ancestry are verified. Never use `-D`, force push, rebase, cherry-pick, or
reset to repair an unexpected graph.

## Standard routine

Start of task:

```bash
git fetch --prune origin
git switch main
git pull --ff-only origin main
git status --short --branch
git branch -vv
git branch -r
git stash list
git worktree list
```

Before merging:

```bash
git diff --check
git diff --stat origin/main...HEAD
git log --graph --decorate --oneline origin/main..HEAD
scripts/assurance/polish.sh
```

After merging:

```bash
git fetch --prune origin
git switch main
git pull --ff-only origin main
git status --porcelain
git branch -vv
git branch -r
git stash list
git worktree list
```

## Dependent branches

Stacked branches are integrated bottom-up. When a foundational PR merges, a
dependent branch is kept intact; if CI strict mode requires freshness, merge
`origin/main` into the dependent branch (creating a merge commit), rerun the
full assurance gate, push, and open its PR. Never rebase the published branch.

## Milestone tags

Annotated tags may be created only after the relevant milestone has landed on
`main` and passed its release gates (for example `recovery-m4` and
`mixed-workload-m5.1`). Individual feature commits are not tagged.

## Never

- Never use `git push --force` or rewrite published commits.
- Never merge a dependent branch before its foundational branch.
- Never delete a branch merely because it looks old.
- Never leave an outdated PR description after implementation changes.
- Never use a ZIP export or secondary working copy as Git history.
- Never let local `main` drift from `origin/main`.
- Never allow undocumented differences between this policy and GitHub branch protection.

## GitHub protection audit

The protection settings are checked against the repository policy:

- direct pushes to `main` restricted; PR required;
- required CI checks and latest-check enforcement enabled;
- force pushes and deletion of `main` disabled;
- merge commit allowed; squash/rebase merging disabled;
- linear history not required (the repository deliberately uses merge commits);
- conversation resolution required;
- required approving reviews remain zero because an artificial reviewer
  requirement cannot be satisfied by the sole developer; self-review plus CI
  is the enforced substitute.
