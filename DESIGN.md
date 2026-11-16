---
version: alpha
colors:
  primary: "#007AFF"
  background: "#F5F5F5"
  text: "#000000"
  darkBackground: "#2E303D"
  darkText: "#FFFFFF"
  secondary: "#FC9B76"
typography:
  body:
    fontFamily: 'Microsoft YaHei UI, Microsoft YaHei, Roboto, Arial, sans-serif'
rounded:
  DEFAULT: "8px"
omitted:
  - section: spacing
    reason: "The existing MUI theme and component conventions own spacing."
  - section: components
    reason: "Existing shared BasePage and MUI components own the interface."
---

## Overview

Clash Verge is a compact desktop network-management tool. Preserve the established functional layout and terminology; the Traffic page is a sibling of Connections, not a visual rebrand.

## Colors

`src/pages/_theme.tsx` owns light and dark palette values, and `src/pages/_layout/hooks/use-custom-theme.ts` applies user customization. Read semantic colors through the MUI theme; use the existing background and scrollbar variables from `src/assets/styles/index.scss`.

## Typography

Use the existing application font stack and theme typography, with native CJK fallbacks. Keep table numerals legible and align data consistently.

## Layout

Reuse the existing left navigation, BasePage header, compact rounded panels and internal scrolling. Application icons lead each row; the chain and current and historical byte counts are the primary information.

Traffic column proportions are owned by `src/features/traffic/lib/traffic-layout.ts` and applied through a fixed-layout semantic table and colgroup. Only viewport size and explicit column visibility changes may resize the columns; live values, long names, routes, icons and expanded details must not affect them. Numeric cells reserve two lines for proxy and inclusive totals, with a stable row height. Long table identifiers use ellipsis and expose their full values in expanded details. The compact WSL status reserves its width, and the storage path has a separate single-line slot. Keep the table's scrollbar gutter stable and horizontal overflow inside the table panel.

## Elevation & Depth

Follow the existing flat, low-shadow desktop surfaces and light/dark theme behavior.

## Shapes

Use the existing 8px page and component radius and MUI control shapes.

## Components

Reuse BasePage, MUI tables, buttons, status and tooltip components. Show empty, disconnected, loading and unrecognized-application states without hiding recorded traffic.

## Do's and Don'ts

Keep density, labels, icon style and interaction behavior consistent with Connections. Do not introduce a separate visual system or imply that sampled counters are exact billing figures.

Traffic primary values, sorting, filters and summaries use proxy bytes. Parentheses show inclusive bytes, including DIRECT. Classify routes by the final outbound. Charts default to proxy traffic and explicitly label any inclusive view. Nonlinear value scales must be labeled, retain raw byte tooltips, and preserve upload/download proportions at each sample. Fit both area boundaries smoothly, fill to those exact curves, keep the boundaries ordered, and leave missing periods disconnected.

Application identity follows a stable installed package or known versioned install root; preserve raw executable paths in expanded details and keep different publishers and installation roots separate. Pins and expansion keys use this stable identity. In one-day charts, usage recovered after an interruption stays outside the timed area and remains included in daily/monthly totals. Keep both fitted boundaries and raw five-second hover values when reducing subpixel vertices. Record management opens in a compact dialog and does not move table controls while statistics refresh.

The Recent active switch follows proxy network activity for the selected current day. Cluster the latest activity of stable application identities at natural gaps, with a five-minute minimum and no fixed maximum. Idle open connections do not renew activity. Preserve pinned applications through this filter, retain other explicit filters, and leave selected-day/month byte totals unchanged. Disable the switch on historical dates while retaining its preference. Follow midnight only when the user was viewing today.

Executable icon recovery may use exact package-manifest executable entries, verified application ancestors and cached authentic icons. Never guess a process from a domain or pick an ambiguous package entry. If a native icon remains unavailable, keep the fixed icon box and show a clearly identified name marker; classify Mihomo internal connections separately from unknown applications.

Traffic storage metadata keeps the full path and physical main-file size in one fixed-height row. The path may ellipsize; the parenthesized size keeps a reserved slot. Record management owns WAL and reusable-space details. The portable-update dialog reuses BaseDialog and the existing theme; its repository action is shown only when a real fork URL is configured.

The settings header keeps the upstream help, Telegram and GitHub actions, followed by the owner's bundled avatar in the same button size. The owner link has an accessible name and no runtime image request. Fork updates show the complete upstream-plus-OTool version, separate checking, failed, unpublished and current/new states, and offer retry or the release page after failures. Release notes scroll inside the existing dialog; download actions identify the complete portable package and explain the folder-switch step.

Startup service checks preserve the user decision for incompatible or unavailable installed services. Keep the existing service-update dialog open until the user authorizes update/repair or explicitly chooses user mode for this session. Explain Windows authorization during the operation, keep cancellation retryable, and confirm success only after observing a usable service and a core running in Service mode.
