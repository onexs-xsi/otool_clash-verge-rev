# Traffic extension architecture

This fork adds application-level traffic history to Clash Verge Rev. The upstream application remains the host: its theme, command transport, mutation funnel, settings persistence, Mihomo lifecycle and navigation conventions still own their existing responsibilities.

## Owners and integration points

| Location | Responsibility |
| --- | --- |
| `src/features/traffic/` | Traffic page, UI components, pure accounting/view logic, polling and clear-history lifetime |
| `src/services/traffic/` | Traffic IPC types/commands, icon request cache, clipboard and file-export intents |
| `src-tauri/src/core/traffic/` | Collector, accounting, SQLite, recognition, icons, read-only WSL probe and traffic command implementations |
| `src/locales/*/traffic.json` | Feature copy; untranslated locales retain the host provider's fallback |
| `scripts/traffic/` | Portable packaging, boundary checks, upstream comparison and Windows test setup |
| `traffic.edition.json` | Fork display name, optional public repository URL, upstream baseline and application-update policy |
| `integration.json` | Explicit list of owned files and justified changes to host files |

Host components use the feature's public exports. Native startup calls `traffic::start()`; shutdown calls `traffic::flush()`. Tauri commands are re-exported through the host's existing command registry. The existing command names and flat Verge preference keys are retained, so restructuring does not reset v7–v9 settings, pins or history.

The public traffic RPC types are in `src/services/traffic/api.ts`. `historyBytes` adds the physical main SQLite file size to the statistics view. Reading this value uses file metadata outside the collector state lock; it does not count sample rows or scan the database on each refresh. WAL and reusable space remain available in Record management.

## Data and ownership constraints

- SQLite counters and connection checkpoints commit in one transaction. The WAL remains part of live storage; copying just the main file is not a safe backup. The existing online backup command owns backups.
- Five-second samples remain separate from bytes recovered without reliable timing after sleep or interruption. Refactoring must not interpolate recovered bytes into a fabricated timeline.
- Primary values, ordering and activity filters use proxy traffic. Inclusive totals retain DIRECT data in parentheses. Sampling estimates are not an authoritative subscription bill.
- Stable application identity, raw executable paths and diagnostic evidence serve different purposes. Keep identity fixtures shared by Rust and TypeScript, preserve raw paths, and do not infer a process from a domain name.
- Recent activity has an adaptive cohort with a five-minute minimum. Idle connections do not refresh activity; pins survive that filter. Day/month totals retain their selected-period meaning.
- WSL monitoring is read-only. It does not start a stopped distribution, enable TUN, write proxy variables, change routing, or edit WSL configuration. The existing UI may ask the user to enable TUN.

`useTrafficHistory` owns statistics polling, interruption/error state and the revision boundary around clearing. Replies from an old date or an earlier poll cannot overwrite a newer selection or a completed clear.

## Fork identity and application updates

The current edition deliberately retains the host identifier, AppData directory and service integration to preserve existing users' profiles and SQLite records. This is not an isolated side-by-side installation of the official client. A future identifier change requires an explicit migration and service-ownership design.

Application updates use complete portable folders. The JavaScript checker, native manual checker, cached startup installer and background checker obey the edition policy. Portable builds also apply an updater overlay with no upstream application endpoint. Mihomo core updates remain owned by the host and continue to work.

The inherited updater public key is not a signing key for this fork. Automated fork updates require a dedicated release feed, signing configuration and validation before changing this policy. Put the real fork URL in `traffic.edition.json` after creating the repository; CI can supply `VITE_TRAFFIC_REPOSITORY` when building its artifacts.

## Continuous verification

`pnpm traffic:check` rejects unrecorded host changes and misplaced feature code. The host's `pnpm arch:check` still enforces event and command ownership. Existing domain tests cover identity, accounting, chart gaps, SQLite durability and activity semantics. `scripts/traffic/test-native.ps1` locates the Cargo test artifact and applies the Windows Common Controls manifest before running feature tests.

The Traffic edition GitHub workflow runs these checks and can build a Windows portable artifact on manual dispatch. It does not publish a GitHub release, send notifications or require release-signing credentials. Cloud execution must be verified in the actual fork.
