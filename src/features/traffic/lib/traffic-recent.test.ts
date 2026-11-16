import { describe, expect, it } from 'vitest'

import type { TrafficStatisticsRow } from '@/services/traffic/api'

import { applicationKey } from './traffic-order'
import { minimumRecentWindow, recentActivity } from './traffic-recent'

const now = 1793358000000
const row = (
  app: string,
  age: number,
  chain = 'AI / JP',
): TrafficStatisticsRow => ({
  app,
  path: `C:\\Apps\\${app}.exe`,
  chain,
  lastActivityAt: now - age,
  active: true,
  dayDurationMs: 0,
  durationIncomplete: false,
  uploadRate: 0,
  downloadRate: 0,
  dayUpload: 1,
  dayDownload: 2,
  monthUpload: 1,
  monthDownload: 2,
})

describe('adaptive recent application activity', () => {
  it('keeps at least five minutes and separates a recent cluster at a natural gap', () => {
    const rows = [row('a', 60_000), row('b', 120_000), row('older', 3_600_000)]
    const recent = recentActivity(rows, now)
    expect(recent.windowMs).toBeGreaterThanOrEqual(minimumRecentWindow)
    expect([...recent.applications]).toEqual(
      rows.slice(0, 2).map(applicationKey),
    )
    expect(rows[0].dayDownload).toBe(2)
  })
  it('expands with a sparse activity cohort instead of applying a fixed time limit', () => {
    const rows = [row('a', 600_000), row('b', 1_200_000), row('c', 1_800_000)]
    expect(recentActivity(rows, now).windowMs).toBeGreaterThan(1_800_000)
    expect(recentActivity(rows, now).applications.size).toBe(3)
  })
  it('uses proxy activity and current transfer, without treating an idle open connection as activity', () => {
    const rows = [
      row('direct', 0, 'DIRECT'),
      { ...row('running', 3_600_000), uploadRate: 1 },
      row('idle', 3_600_000),
    ]
    const recent = recentActivity(rows, now)
    expect([...recent.applications]).toEqual([applicationKey(rows[1])])
    expect(recentActivity([], now).windowMs).toBe(minimumRecentWindow)
    expect(
      recentActivity([{ ...row('unknown-time', 0), lastActivityAt: null }], now)
        .applications.size,
    ).toBe(0)
  })
})
