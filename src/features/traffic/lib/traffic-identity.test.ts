import { describe, expect, it } from 'vitest'

import type { TrafficStatisticsRow } from '@/services/traffic/api'

import fixtures from './__fixtures__/application-identities.json'
import {
  applicationIdentity,
  normalizeApplicationKey,
} from './traffic-identity'
import { groupTrafficRows, orderTrafficRows } from './traffic-order'

describe('application identity across upgrades', () => {
  it.each(fixtures)(
    'keeps the backend identity for $path',
    ({ app, path, identity }) => {
      expect(applicationIdentity(app, path)).toEqual(identity)
      expect(normalizeApplicationKey(JSON.stringify([app, path]))).toBe(
        JSON.stringify(identity),
      )
      expect(normalizeApplicationKey(JSON.stringify(identity))).toBe(
        JSON.stringify(identity),
      )
    },
  )

  it('merges installed versions and chains, keeps current icons and transfers an old pin', () => {
    const row = (
      fixture: (typeof fixtures)[number],
      chain: string,
      download: number,
      active = false,
    ): TrafficStatisticsRow => ({
      app: fixture.app,
      path: fixture.path,
      chain,
      active,
      dayDownload: download,
      dayUpload: 1,
      dayDurationMs: 5000,
      durationIncomplete: false,
      downloadRate: 0,
      uploadRate: 0,
      monthDownload: download,
      monthUpload: 1,
    })
    const old = row(fixtures[0], 'AI / Japan', 100)
    const current = row(fixtures[1], 'AI / Japan', 200, true)
    const direct = row(fixtures[1], 'DIRECT', 300, true)
    const other = row(fixtures[5], 'AI / Japan', 400)
    const merged = groupTrafficRows([old, current, direct, other])
    expect(merged).toHaveLength(2)
    expect(merged[0]).toMatchObject({
      path: current.path,
      dayDownload: 300,
      all: { dayDownload: 600 },
      chains: ['AI / Japan', 'DIRECT'],
    })
    expect(groupTrafficRows([old, current, direct], true)).toHaveLength(2)
    const pin = normalizeApplicationKey(JSON.stringify([old.app, old.path]))
    const ordered = orderTrafficRows(merged, [pin], {
      field: 'dayTotal',
      direction: 'desc',
    })
    expect(ordered.pinned).toHaveLength(1)
    expect(ordered.pinned[0].path).toBe(current.path)
  })
})
