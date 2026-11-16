import type { TrafficStatisticsRow } from '@/services/traffic/api'

import { trafficRoute } from './traffic-metrics'
import { applicationKey } from './traffic-order'

export const minimumRecentWindow = 5 * 60_000

export function recentActivity(rows: TrafficStatisticsRow[], now: number) {
  const latest = new Map<string, number>()
  for (const row of rows) {
    if (trafficRoute(row.chain) !== 'proxy') continue
    const at = row.uploadRate + row.downloadRate > 0 ? now : row.lastActivityAt
    if (at == null || !Number.isFinite(at) || at <= 0) continue
    const key = applicationKey(row)
    latest.set(key, Math.max(latest.get(key) ?? 0, Math.min(now, at)))
  }
  const times = [...latest.values()].sort((a, b) => b - a)
  let oldest = times.at(-1) ?? now
  for (let index = 0; index + 1 < times.length; index += 1) {
    const newerAge = Math.max(minimumRecentWindow, now - times[index])
    const olderAge = now - times[index + 1]
    // A large relative gap separates a recent activity cohort from older applications.
    if (
      times[index] - times[index + 1] >= minimumRecentWindow &&
      olderAge >= 4 * newerAge
    ) {
      oldest = times[index]
      break
    }
  }
  const cutoff = Math.min(
    now - minimumRecentWindow,
    oldest - minimumRecentWindow,
  )
  return {
    windowMs: now - cutoff,
    applications: new Set(
      [...latest].filter(([, at]) => at >= cutoff).map(([key]) => key),
    ),
  }
}
