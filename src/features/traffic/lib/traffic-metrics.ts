import type { TrafficStatisticsRow } from '@/services/traffic/api'

export type TrafficRoute = 'proxy' | 'direct' | 'rejected' | 'unknown'

export const formatTrafficDuration = (
  milliseconds: number | null,
  incomplete: boolean,
) => {
  if (milliseconds == null) return '—'
  const seconds = Math.floor(milliseconds / 1000)
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  return `${incomplete ? '≥ ' : ''}${hours ? `${hours}h ` : ''}${minutes ? `${minutes}m ` : ''}${seconds % 60}s`
}
export type TrafficByteField =
  | 'uploadRate'
  | 'downloadRate'
  | 'dayUpload'
  | 'dayDownload'
  | 'monthUpload'
  | 'monthDownload'
export const trafficByteFields: TrafficByteField[] = [
  'uploadRate',
  'downloadRate',
  'dayUpload',
  'dayDownload',
  'monthUpload',
  'monthDownload',
]

export function trafficRoute(chain: string): TrafficRoute {
  const terminal = chain.split(' / ').at(-1)?.trim() ?? ''
  if (terminal === 'DIRECT') return 'direct'
  if (terminal === 'REJECT' || terminal === 'REJECT-DROP') return 'rejected'
  if (!terminal || terminal === 'PASS' || terminal === 'COMPATIBLE')
    return 'unknown'
  return 'proxy'
}

export type TrafficMetricRow = TrafficStatisticsRow & {
  all: TrafficStatisticsRow
  chains?: string[]
}

export function proxyTrafficRow(row: TrafficStatisticsRow): TrafficMetricRow {
  if (trafficRoute(row.chain) === 'proxy') return { ...row, all: row }
  return {
    ...row,
    ...Object.fromEntries(trafficByteFields.map((field) => [field, 0])),
    dayDurationMs: 0,
    durationIncomplete: false,
    lastActivityAt: null,
    all: row,
  }
}

export function sumTrafficRows(
  rows: TrafficStatisticsRow[],
): TrafficStatisticsRow {
  const first = rows[0]
  const result: TrafficStatisticsRow = {
    app: first?.app ?? '',
    path: first?.path ?? '',
    chain: '',
    uploadRate: 0,
    downloadRate: 0,
    dayUpload: 0,
    dayDownload: 0,
    monthUpload: 0,
    monthDownload: 0,
    dayDurationMs: null,
    durationIncomplete: false,
  }
  let durations = 0
  for (const row of rows) {
    if (row.lastActivityAt != null)
      result.lastActivityAt = Math.max(
        result.lastActivityAt ?? 0,
        row.lastActivityAt,
      )
    for (const field of trafficByteFields) result[field] += row[field]
    if (row.dayDurationMs != null) {
      result.dayDurationMs = Math.max(
        result.dayDurationMs ?? 0,
        row.dayDurationMs,
      )
      if (row.dayDurationMs > 0) durations += 1
    }
    result.durationIncomplete ||= row.durationIncomplete
  }
  // Only a lower bound is available when chains have overlapping activity.
  result.durationIncomplete ||= durations > 1
  return result
}

export function trafficTotals(rows: TrafficStatisticsRow[]) {
  const proxyRows = rows.filter((row) => trafficRoute(row.chain) === 'proxy')
  const proxy = sumTrafficRows(proxyRows)
  if (proxyRows.length === 0) proxy.dayDurationMs = 0
  return { proxy, all: sumTrafficRows(rows) }
}

export const hasProxyTraffic = (row: TrafficStatisticsRow) =>
  row.dayUpload +
    row.dayDownload +
    row.monthUpload +
    row.monthDownload +
    row.uploadRate +
    row.downloadRate >
  0
