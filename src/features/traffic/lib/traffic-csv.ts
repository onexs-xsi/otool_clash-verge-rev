import type { TrafficMetricRow } from './traffic-metrics'

const trafficCsvFields = [
  'app',
  'path',
  'chain',
  'day',
  'month',
  'proxyDayDownload',
  'proxyDayUpload',
  'proxyDayTotal',
  'allDayDownload',
  'allDayUpload',
  'allDayTotal',
  'proxyMonthDownload',
  'proxyMonthUpload',
  'allMonthDownload',
  'allMonthUpload',
  'proxyDownloadRate',
  'proxyUploadRate',
  'allDownloadRate',
  'allUploadRate',
  'proxyDurationMs',
  'proxyDurationLowerBound',
] as const

const csvCell = (value: string | number | boolean | null) => {
  let text = value == null ? '' : String(value)
  if (typeof value === 'string' && /^[=+\-@\t\r]/.test(text)) text = `'${text}`
  return `"${text.replaceAll('"', '""')}"`
}

export function trafficCsv(
  rows: TrafficMetricRow[],
  day: string,
  month: string,
  labels: string[] = [...trafficCsvFields],
) {
  const lines = rows.map((row) =>
    [
      row.app,
      row.path,
      row.chains?.join(' | ') ?? row.chain,
      day,
      month,
      row.dayDownload,
      row.dayUpload,
      row.dayDownload + row.dayUpload,
      row.all.dayDownload,
      row.all.dayUpload,
      row.all.dayDownload + row.all.dayUpload,
      row.monthDownload,
      row.monthUpload,
      row.all.monthDownload,
      row.all.monthUpload,
      row.downloadRate,
      row.uploadRate,
      row.all.downloadRate,
      row.all.uploadRate,
      row.dayDurationMs,
      row.durationIncomplete,
    ]
      .map(csvCell)
      .join(','),
  )
  return `\uFEFF${[labels.map(csvCell).join(','), ...lines].join('\r\n')}\r\n`
}
