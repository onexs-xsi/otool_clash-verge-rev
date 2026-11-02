import { expect, it, vi } from 'vitest'

import { getTrafficStatistics } from './api'

const getStatistics = vi.hoisted(() => vi.fn())

vi.mock('@/services/bindings', () => ({
  commands: { getTrafficStatistics: getStatistics },
}))

it('normalizes nullable IPC rates while preserving recorded counters', async () => {
  const row = {
    app: 'example.exe',
    path: 'C:\\Apps\\example.exe',
    chain: 'Proxy / JP',
    active: true,
    lastActivityAt: null,
    dayDurationMs: 5000,
    durationIncomplete: false,
    uploadRate: null,
    downloadRate: 12.5,
    dayUpload: 1024,
    dayDownload: 2048,
    monthUpload: 4096,
    monthDownload: 8192,
  }
  const view = {
    connected: true,
    loaded: true,
    historyError: null,
    historyPath: 'C:\\Data\\traffic\\history.sqlite3',
    historyBytes: 4096,
    rows: [row, { ...row, uploadRate: 25, downloadRate: null }],
  }
  getStatistics.mockResolvedValueOnce(view)

  const actual = await getTrafficStatistics('2026-11-03', '2026-11')

  expect(getStatistics).toHaveBeenCalledWith('2026-11-03', '2026-11')
  expect(actual).toEqual({
    ...view,
    rows: [
      { ...row, uploadRate: 0 },
      { ...row, uploadRate: 25, downloadRate: 0 },
    ],
  })
})
