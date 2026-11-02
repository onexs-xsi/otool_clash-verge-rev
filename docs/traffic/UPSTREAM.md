# Keeping the fork close to upstream

The recorded baseline is `upstream.baseRevision` in `traffic.edition.json`. Upstream application versions and Traffic edition numbers are separate: importing a new host version must not rename or overwrite an existing portable edition.

## Review before merging

Keep `origin` for the actual fork and add the original repository as `upstream` when setting up the fork:

```sh
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git fetch upstream
pnpm traffic:upstream --ref upstream/main
```

The last command is read-only. It compares commits against the recorded baseline and reports upstream changes to the documented integration files, together with each file's purpose. It does not fetch, merge, discard local changes or claim that a same-file change is necessarily a merge conflict.

Commit the feature work on the fork before synchronizing. Create a sync branch, merge upstream and resolve host adapters with reference to `integration.json`. Keep domain behavior in the feature-owned directories rather than growing host patches to make a conflict disappear.

Focus on these contracts during a merge:

1. Tauri command registration, transport payloads and flat preference patches.
2. Mihomo connection-stream lifetime and startup/shutdown ordering.
3. Navigation migration, feature locale loading and theme ownership.
4. SQLite transaction/checkpoint behavior, five-second samples and interruption recovery.
5. Manual fork updates, shared AppData identity and read-only WSL behavior.

After validation, update the baseline to the upstream commit actually merged. Do not point it at a fork commit merely to hide unexplained host changes.

## Validate and build

Use the upstream dependency preparation process for a fresh checkout, then run:

```sh
pnpm traffic:check
pnpm arch:check
pnpm typecheck
pnpm test
pnpm traffic:test:package
```

On Windows, run the native feature tests with a Windows SDK installed:

```powershell
./scripts/traffic/test-native.ps1
pnpm portable --version 11
```

Use the next available edition number. The packager writes directly to `releases/archive/vN`, refuses to replace an existing edition and leaves a running old version's files alone. It uses the current checkout's upstream `package.json` version for host metadata and embeds the Traffic edition for the manual-update dialog.

The full portable folder, including resources and cores, is the release unit. Source and documentation belong in Git; dependencies, local history, icon caches, temporary probes, binary archives and validation reports do not. Retain the upstream license and attribution.

Use a fork release tag such as `traffic-v10`. The inherited official nightly and release entry jobs are restricted to the upstream repository; the Traffic edition workflow owns unsigned portable fork artifacts. Other inherited maintenance workflows remain available for deliberate manual use and may require their original configuration.
