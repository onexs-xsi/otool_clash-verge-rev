import { describe, expect, it } from 'vitest'

import type { TrafficStatisticsRow } from '@/services/traffic/api'

import {
  applicationKey,
  groupTrafficRows,
  orderTrafficRows,
} from './traffic-order'

const row = (
  app: string,
  chain: string,
  downloadRate: number,
): TrafficStatisticsRow => ({
  app,
  path: `${app}.exe`,
  chain,
  dayDurationMs: null,
  durationIncomplete: false,
  downloadRate,
  uploadRate: 0,
  dayUpload: 0,
  dayDownload: 0,
  monthUpload: 0,
  monthDownload: 0,
})

describe('traffic ordering', () => {
  it('keeps all chains of pinned applications ahead in pin order while sorting only other applications', () => {
    const rows = [
      row('a', 'two', 1),
      row('b', 'direct', 3),
      row('a', 'one', 8),
      row('c', 'proxy', 6),
    ]
    const pinned = [applicationKey(rows[1]), applicationKey(rows[0])]
    const result = orderTrafficRows(rows, pinned, {
      field: 'downloadRate',
      direction: 'desc',
    })
    expect(result.pinned.map((item) => `${item.app}/${item.chain}`)).toEqual([
      'b/direct',
      'a/one',
      'a/two',
    ])
    expect(result.unpinned.map((item) => item.app)).toEqual(['c'])
    expect(
      orderTrafficRows(rows, pinned, {
        field: 'downloadRate',
        direction: 'asc',
      }).pinned,
    ).toEqual(result.pinned)
    expect(rows[0].chain).toBe('two')
  })

  it('sorts numeric values numerically and application names by text', () => {
    const rows = [row('z', 'one', 10), row('a', 'one', 2), row('c', 'one', 100)]
    expect(
      orderTrafficRows(rows, [], {
        field: 'downloadRate',
        direction: 'desc',
      }).unpinned.map((item) => item.downloadRate),
    ).toEqual([100, 10, 2])
    expect(
      orderTrafficRows(rows, [], {
        field: 'app',
        direction: 'asc',
      }).unpinned.map((item) => item.app),
    ).toEqual(['a', 'c', 'z'])
  })

  it('sorts the day total as download plus upload', () => {
    const first = { ...row('a', 'one', 0), dayDownload: 8, dayUpload: 1 }
    const second = { ...row('b', 'one', 0), dayDownload: 2, dayUpload: 20 }
    expect(
      orderTrafficRows([first, second], [], {
        field: 'dayTotal',
        direction: 'desc',
      }).unpinned.map((item) => item.app),
    ).toEqual(['b', 'a'])
  })

  it('groups only matching applications, keeps filtered chains, and never sums overlapping durations', () => {
    const first = {
      ...row('browser', 'DIRECT', 10),
      dayDurationMs: 4000,
      dayDownload: 20,
    }
    const second = {
      ...row('browser', 'Proxy', 30),
      dayDurationMs: 6000,
      dayDownload: 50,
    }
    const other = row('other', 'DIRECT', 5)
    const grouped = groupTrafficRows([first, second, other])
    expect(grouped).toHaveLength(2)
    expect(grouped[0]).toMatchObject({
      app: 'browser',
      downloadRate: 30,
      dayDownload: 50,
      dayDurationMs: 6000,
      durationIncomplete: false,
      chains: ['DIRECT', 'Proxy'],
      all: {
        downloadRate: 40,
        dayDownload: 70,
        dayDurationMs: 6000,
        durationIncomplete: true,
      },
    })
    expect(groupTrafficRows([second])[0]).toMatchObject({
      dayDurationMs: 6000,
      durationIncomplete: false,
      chains: ['Proxy'],
    })
    expect(first.dayDownload).toBe(20)
  })

  it('sorts by proxy bytes even when another application has much more direct traffic', () => {
    const rows = [
      {
        ...row('windows', 'Microsoft / DIRECT', 100),
        dayDownload: 731 * 1024 * 1024,
      },
      { ...row('chat', 'AI / Japan', 20), dayDownload: 10 * 1024 * 1024 },
      {
        ...row('browser', 'Video / DIRECT', 200),
        dayDownload: 25 * 1024 * 1024,
      },
      { ...row('browser', 'AI / Japan', 2), dayDownload: 100 * 1024 },
    ]
    const sorted = orderTrafficRows(groupTrafficRows(rows), [], {
      field: 'dayDownload',
      direction: 'desc',
    }).unpinned
    expect(sorted.map((item) => item.app)).toEqual([
      'chat',
      'browser',
      'windows',
    ])
    expect(sorted[1].dayDownload).toBe(100 * 1024)
    expect(sorted[1].all.dayDownload).toBe(25 * 1024 * 1024 + 100 * 1024)
    expect(sorted[2].dayDownload).toBe(0)
  })
})
