# GitHub branch-protection audit

- Audit date: 2026-11-17
- Repository: `siebengestirn/engineering-challenge-template`
- Main branch: `main` (local `9ba0141` at audit time, in sync with `origin/main`)
- Auditor: repository maintainer (self-audit; single-developer repository)
- Evidence: `gh api repos/siebengestirn/engineering-challenge-template/branches/main/protection`

| Setting | Observed | Policy expectation | Match |
|---|---|---|---|
| Direct push restriction | enabled (`enforce_admins: true`) | enabled | yes |
| Pull request required | enabled (required PR reviews object present, 0 approvals) | enabled | yes |
| Required CI checks | `Format, lint, test, release build`; `Traceability and Mermaid validation` | enabled | yes |
| Strict freshness | `required_status_checks.strict: true` | enabled | yes |
| Force pushes | `allow_force_pushes.enabled: false` | disabled | yes |
| Main deletion | `allow_deletions.enabled: false` | disabled | yes |
| Linear history | `required_linear_history.enabled: false` | disabled (merge-commit policy) | yes |
| Conversation resolution | enabled | enabled | yes |
| Merge methods | merge-commit PRs used for #45 and #47; squash/rebase not used | merge commit only | yes |

## Notes

- `merge_commit_allowed` / `squash_merge_allowed` / `rebase_merge_allowed` are
  not exposed by the used API view; the actual integration method is evidenced
  by PR #45 (`d54aa08`) and PR #47 (`9ba0141`), both merge commits.
- Required approving reviews remain zero by policy: an artificial reviewer
  requirement cannot be satisfied by the sole developer. The enforced
  substitute is an explicit self-review checklist plus independent CI on the
  exact PR head.
- PR #45 (`d54aa08`) and PR #47 (`9ba0141`) are both `MERGED`; PR #45 was
  reconciled from its obsolete draft state and its current description records
  the complete delivered scope. `c480da8` is an ancestor of `origin/main`.
  No open PRs remain; a provenance comment on PR #45 records the merge chain.
- This record must be refreshed after any material branch-protection change.
