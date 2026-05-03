# AQBot Independent Evolution Guide

This repo now follows a "fork-first, upstream-aware" workflow:

- `origin` (`git@github.com:yuholy/AQBot.git`) is the source of truth for product work.
- `upstream` (`git@github.com:AQBot-Desktop/AQBot.git`) is kept only for review, selective sync, and reference.
- `yuholy` is the long-lived product branch for ongoing independent evolution.

## Working Rules

1. Build new product capabilities on `yuholy` or short-lived branches created from `yuholy`.
2. Do not treat `upstream/main` as a branch that must always be matched.
3. Only pull from upstream when the change is clearly valuable:
   - security fixes
   - crash or correctness fixes
   - dependency or platform compatibility updates
   - reusable low-level infrastructure improvements
4. Avoid syncing upstream product decisions by default:
   - page layout changes
   - interaction rewrites
   - feature directions that diverge from AQBot's product roadmap
5. Prefer solving integration needs in the main repo before editing submodules.

## Branch Strategy

- `main`: keep as a clean baseline branch when useful for comparison or release alignment.
- `yuholy`: primary product trunk.
- feature branches: create from `yuholy` and merge back into `yuholy`.

Recommended examples:

- `feature/vehicle-module-polish`
- `feature/personal-agent-memory`
- `fix/chat-page-switch-lag`

## Upstream Sync Policy

Default policy: inspect first, merge later.

Safe review cycle:

1. Fetch both remotes.
2. Compare `yuholy` with `upstream/main`.
3. Read the upstream commits.
4. Cherry-pick or manually port only the changes we want.
5. Validate locally before pushing.

Preferred integration order:

1. `git fetch origin --prune`
2. `git fetch upstream --prune`
3. `git log --oneline yuholy..upstream/main`
4. `git diff --stat yuholy..upstream/main`
5. selectively `git cherry-pick <commit>` or reimplement by hand

Avoid this as a default habit:

- `git merge upstream/main`
- `git rebase upstream/main`

Those commands are not forbidden forever, but they should be rare and intentional because they can import a large amount of upstream product coupling.

## Ownership Boundaries

Keep these in the main repo whenever possible:

- product UX and page flow
- chat behavior
- knowledge and memory orchestration
- external agent integration
- provider routing
- settings and persistence policies
- task dispatch and result ingestion

Treat these as submodule-change exceptions, not defaults:

- `src-tauri/crates/open-agent-sdk`
- `libs/markstream-vue`
- `libs/stream-monaco`

Only touch a submodule when all three are true:

1. the main repo cannot reasonably wrap the issue
2. the limitation is inside the submodule itself
3. the value of the change outweighs the maintenance cost

## Decision Filter

Before syncing from upstream, ask:

1. Is this a bug fix, security fix, or platform fix?
2. Does it reduce maintenance burden in our fork?
3. Will it conflict with AQBot's current roadmap or UX direction?
4. Can we port only the needed slice instead of merging everything?

If the answer to 1 or 2 is "no", default to not syncing.
If the answer to 3 is "yes", default to not syncing.
If the answer to 4 is "yes", prefer selective porting.

## Monthly Maintenance Rhythm

- once a week or once every two weeks: inspect upstream delta
- once a month: decide whether there is a worthwhile sync batch
- before major releases: re-check upstream for security, Tauri, Rust, or dependency fixes

## Local Commands

Use the review script:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\review-upstream.ps1
```

Compare another branch against upstream:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\review-upstream.ps1 -BaseBranch main
```

## Current Operating Position

As of now, this repo should be treated as an independently evolving product fork.
Upstream remains a technical reference, not the primary roadmap owner.
