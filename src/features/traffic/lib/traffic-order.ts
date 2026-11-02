import type { TrafficStatisticsRow } from '@/services/traffic/api'

import { applicationIdentity } from './traffic-identity'
import { trafficTotals, type TrafficMetricRow } from './traffic-metrics'

export type TrafficSortField =
  | Exclude<
      keyof TrafficStatisticsRow,
      'path' | 'durationIncomplete' | 'active' | 'lastActivityAt'
    >
  | 'dayTotal'
export interface TrafficSort {
  field: TrafficSortField
  direction: 'asc' | 'desc'
}

export const applicationKey = (
  row: Pick<TrafficStatisticsRow, 'app' | 'path'>,
) => JSON.stringify(applicationIdentity(row.app, row.path))

export type TrafficDisplayRow = TrafficMetricRow

export function groupTrafficRows(
  rows: TrafficStatisticsRow[],
  byChain = false,
): TrafficDisplayRow[] {
  const groups = new Map<string, TrafficStatisticsRow[]>()
  for (const row of rows) {
    const key = JSON.stringify([
      applicationKey(row),
      byChain ? row.chain : null,
    ])
    const current = groups.get(key)
    if (current) current.push(row)
    else groups.set(key, [row])
  }
  return [...groups.values()].map((items) => {
    const { proxy, all } = trafficTotals(items)
    const chains = [...new Set(items.map((row) => row.chain))].sort()
    const representative = items
      .slice()
      .sort(
        (a, b) =>
          Number(Boolean(b.active)) - Number(Boolean(a.active)) ||
          b.downloadRate + b.uploadRate - (a.downloadRate + a.uploadRate) ||
          b.path.localeCompare(a.path, undefined, { numeric: true }),
      )[0]
    return {
      ...proxy,
      app: representative.app,
      path: representative.path,
      active: items.some((row) => row.active),
      chain: chains.join(' | '),
      chains: byChain ? undefined : chains,
      all,
    }
  })
}

export function orderTrafficRows<T extends TrafficStatisticsRow>(
  rows: T[],
  pinnedKeys: string[],
  sort: TrafficSort | null,
) {
  const indexes = new Map(pinnedKeys.map((key, index) => [key, index]))
  const pinned: T[] = []
  const unpinned: T[] = []
  for (const row of rows)
    (indexes.has(applicationKey(row)) ? pinned : unpinned).push(row)
  pinned.sort(
    (a, b) =>
      (indexes.get(applicationKey(a)) ?? 0) -
        (indexes.get(applicationKey(b)) ?? 0) || a.chain.localeCompare(b.chain),
  )
  if (sort) {
    const factor = sort.direction === 'asc' ? 1 : -1
    unpinned.sort((a, b) => {
      const left =
        sort.field === 'dayTotal' ? a.dayDownload + a.dayUpload : a[sort.field]
      const right =
        sort.field === 'dayTotal' ? b.dayDownload + b.dayUpload : b[sort.field]
      if (left == null || right == null) {
        if (left == null && right == null) return 0
        return left == null ? 1 : -1
      }
      const value =
        typeof left === 'number' && typeof right === 'number'
          ? left - right
          : String(left).localeCompare(String(right), undefined, {
              numeric: true,
              sensitivity: 'base',
            })
      return (
        factor * value ||
        applicationKey(a).localeCompare(applicationKey(b)) ||
        a.chain.localeCompare(b.chain)
      )
    })
  }
  return { pinned, unpinned }
}
