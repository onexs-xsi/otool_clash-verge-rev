import { expect, it } from 'vitest'

import { trafficCsv } from './traffic-csv'
import { groupTrafficRows } from './traffic-order'

it('exports separate numeric proxy and inclusive totals and escapes spreadsheet formulas', () => {
  const base = {
    app: '=APP()',
    path: 'C:\\apps\\a.exe',
    dayDurationMs: 10,
    durationIncomplete: false,
    uploadRate: 0,
    downloadRate: 0,
    dayUpload: 3,
    monthUpload: 3,
    monthDownload: 0,
  }
  const rows = groupTrafficRows([
    { ...base, chain: 'DIRECT', dayDownload: 500 },
    { ...base, chain: 'AI / "Japan"', dayDownload: 100 },
  ])
  const csv = trafficCsv(rows, '2026-10-30', '2026-10')
  expect(csv.startsWith('\uFEFF')).toBe(true)
  expect(csv).toContain('"\'=APP()"')
  expect(csv).toContain('AI / ""Japan""')
  expect(csv).toContain('"100","3","103","600","6","606"')
})
