# Issue tracker

Issues and specs live in GitHub Issues for
https://github.com/oDestroyeRo/opentrickster.

Use the gh CLI. Target oDestroyeRo/opentrickster explicitly
with --repo for issue and label commands.

## Operations

- Verify the repository with `gh repo view --json nameWithOwner`.
- Create issues with `gh issue create --title "<title>" --body-file <file>`.
- Read issues with `gh issue view <number> --json number,title,body,labels,comments`.
- List issues with `gh issue list --state open --json number,title,labels`.
- Apply labels with `gh issue edit <number> --add-label "<label>"`.
- Remove labels with `gh issue edit <number> --remove-label "<label>"`.
- Before closing an issue, post the reason with
  `gh issue comment <number> --body-file <file>`.
  Then run `gh issue close <number>`.

Use native GitHub sub-issues and dependencies when available.
Otherwise record `Part of #<parent>` and `Blocked by: #<number>`
in the issue body. Work is unblocked when every blocker is closed.

Create issues only in this repository.

## Skill instructions

"Publish to the issue tracker" means create a GitHub issue.
"Fetch the relevant ticket" means read the GitHub issue.

## Pull requests as a triage surface

PRs as a request surface: no.
