# Upstream Master Sync Conflict

Automated upstream master sync failed while replaying the fork patch stack.

- Upstream master: `d13a30f4aed1dbcbb297347e0012497e6ccec1ae`
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

1. Create a local branch from `d13a30f4aed1dbcbb297347e0012497e6ccec1ae`.
2. Cherry-pick `upstream-master..fork` in order.
3. Resolve conflicts and push a replacement `sync/upstream-master-master-d13a30f4aed1` preview branch.
4. Keep `fork` unchanged until a reviewed `/promote-fork` succeeds.
