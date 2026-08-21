# Upstream Master Sync Conflict

Automated upstream master sync failed while replaying the fork patch stack.

- Upstream master: `155889bbf8338cfe7787fc849416db1c5e286727`
- Previous upstream anchor: `upstream-master` (`21334d424a9ba12cb5011692166758e1e85c7c5c`)
- Current fork branch: `fork` (`9d2455aa08df99f4274ba7709588cc713e6c646d`)
- Failed fork patch: `9d2455aa08df99f4274ba7709588cc713e6c646d` Add CLI agent tab status colors

## Conflicted Files

- `app/src/tab.rs`
- `app/src/terminal/event.rs`
- `app/src/terminal/model/ansi/mod_tests.rs`
- `app/src/terminal/model/terminal_model.rs`
- `app/src/terminal/model_events.rs`
- `app/src/workspace/view.rs`
- `app/src/workspace/view_tests.rs`

## Patch Stack

- `9d2455aa08df99f4274ba7709588cc713e6c646d` Add CLI agent tab status colors

## Manual Handling

1. Create a local branch from `155889bbf8338cfe7787fc849416db1c5e286727`.
2. Cherry-pick `upstream-master..fork` in order.
3. Resolve conflicts and push a replacement `sync/upstream-master-master-155889bbf833` preview branch.
4. Keep `fork` unchanged until a reviewed `/promote-fork` succeeds.
