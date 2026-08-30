# Upstream Master Sync Conflict

Automated upstream master sync failed while replaying the fork patch stack.

- Upstream master: `86cfeb9006da7865d7f27f33228ed0f581d49f02`
- Previous upstream anchor: `upstream-master` (`21334d424a9ba12cb5011692166758e1e85c7c5c`)
- Current fork branch: `fork` (`9d2455aa08df99f4274ba7709588cc713e6c646d`)
- Failed fork patch: `9d2455aa08df99f4274ba7709588cc713e6c646d` Add CLI agent tab status colors

## Conflicted Files

- `app/src/tab.rs`
- `app/src/terminal/event.rs`
- `app/src/terminal/model/ansi/mod.rs`
- `app/src/terminal/model/terminal_model.rs`
- `app/src/terminal/model_events.rs`
- `app/src/workspace/view.rs`
- `app/src/workspace/view_tests.rs`
- `crates/warp_terminal/src/model/ansi/mod_tests.rs`

## Patch Stack

- `9d2455aa08df99f4274ba7709588cc713e6c646d` Add CLI agent tab status colors

## Manual Handling

1. Create a local branch from `86cfeb9006da7865d7f27f33228ed0f581d49f02`.
2. Cherry-pick `upstream-master..fork` in order.
3. Resolve conflicts and push a replacement `sync/upstream-master-master-86cfeb9006da` preview branch.
4. Keep `fork` unchanged until a reviewed `/promote-fork` succeeds.
