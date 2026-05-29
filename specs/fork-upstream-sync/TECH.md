# Warp Fork Upstream Sync Model

## Upstream Shape

Warp upstream uses `master` as the active development branch. GitHub releases are
cut by repository workflows into channel-specific release tags and branches such
as `v0.2026.06.01.10.01.dev_00`, `preview_release/*`, and `stable_release/*`.
Those release artifacts depend on upstream-only infrastructure such as official
signing, channel version publishing, repository sync, and private/internal
secrets.

This fork should therefore track upstream `master`, not the latest release tag.
The `sub2api` fork tracks release tags because that upstream has release-tag
specific follow-up commits. That assumption does not apply to Warp.

## Branch Roles

- `fork`: production fork branch. This is the default branch and the branch that
  contains fork-only patches, including the CLI-agent tab status colors.
- `upstream-master`: anchor branch recording the upstream `master` commit that
  the current `fork` patch stack is based on.
- `sync/upstream-master-<ref>-<sha>`: generated preview branch. It starts from a
  newer upstream `master` commit and replays the fork patch stack from
  `upstream-master..fork` by cherry-picking in order.
- `sync/conflict-upstream-master-<ref>-<sha>`: generated conflict report branch.
  It does not update the fork. It carries a markdown report and opens a draft PR
  when the replay cannot be completed automatically.

## Sync Flow

1. Fetch upstream `warpdotdev/warp` `master`, `fork`, and `upstream-master`.
2. If `upstream-master` already equals upstream `master`, exit successfully.
3. Build the fork patch stack with `git rev-list --reverse
   origin/upstream-master..origin/fork`.
4. Create a preview branch at the new upstream `master` commit.
5. Cherry-pick each fork patch in order with `--empty=drop`.
6. On conflict, create or update a draft conflict PR with the failed commit,
   conflicted files, and manual handling instructions. The workflow still exits
   successfully if it publishes that PR, because the durable artifact is the PR
   rather than a red scheduled run.
7. On success, push the preview branch, open or update a draft PR to `fork`, and
   dispatch fork-safe preview checks.
8. A maintainer promotes the preview only after review and checks by commenting
   `/promote-fork` on the PR. The promote workflow updates `fork` to the preview
   head and moves `upstream-master` to the upstream baseline with
   `--force-with-lease`.

## Fork Workflow Policy

Most upstream workflows are not fork-safe because they require official Warp
infrastructure. The fork should keep only workflows needed for fork operation:

- fork upstream preview sync;
- promote reviewed previews;
- focused fork CI;
- self-signed OSS macOS release.

Upstream-only workflows should be disabled in the fork repository instead of
left red on schedules.
