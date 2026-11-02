import { describe, expect, it } from 'vitest'

import {
  chartBuckets,
  stackedAreas,
  stackedBars,
  timeAxisTicks,
  valueAxisTicks,
  windowChartBuckets,
} from './traffic-series'

describe('traffic chart buckets', () => {
  it('keeps five-second records and short-window timestamps through the end of the day', () => {
    const buckets = chartBuckets('2026-10-30', 1, [
      { period: '2026-10-30T23:59:50', upload: 10, download: 20 },
      { period: '2026-10-30T23:59:55', upload: 30, download: 40 },
    ])
    expect(buckets).toHaveLength(17280)
    expect(buckets.at(-1)).toMatchObject({
      label: '23:59:55',
      upload: 30,
      download: 40,
    })
    const hour = windowChartBuckets(buckets, 60)
    expect(hour).toHaveLength(720)
    expect(hour[0].key).toBe('2026-10-30T23:00:00')
    expect(
      timeAxisTicks(hour, 1, 1200).every(
        (tick) => !tick.label?.includes('NaN'),
      ),
    ).toBe(true)
  })
  it('uses hourly data and groups three-day periods in three-hour intervals', () => {
    const points = [
      { period: '2026-10-28T00', download: 2, upload: 3 },
      { period: '2026-10-28T02', download: 5, upload: 7 },
    ]
    const buckets = chartBuckets('2026-10-30', 3, points)
    expect(buckets).toHaveLength(24)
    expect(buckets[0]).toMatchObject({
      key: '2026-10-28T00',
      download: 7,
      upload: 10,
    })
    expect(buckets[1].download).toBeNull()
    expect(chartBuckets('2026-10-30', 1, points)).toHaveLength(17280)
  })
  it('keeps fine samples intact when an hour also has recovered usage', () => {
    const buckets = chartBuckets('2026-10-30', 1, [
      { period: '2026-10-30T17', upload: 1000, download: 2000 },
      { period: '2026-10-30T17:15:00', upload: 30, download: 80 },
      { period: '2026-10-30T17:15:05', upload: 7, download: 9 },
    ])
    expect(buckets).toHaveLength(17280)
    expect(buckets[17 * 720].upload).toBeNull()
    expect(buckets[(17 * 60 + 15) * 12]).toMatchObject({
      span: 1,
      upload: 30,
      download: 80,
    })
    expect(buckets[(17 * 60 + 15) * 12 + 1].upload).toBe(7)
    expect(buckets.reduce((sum, bucket) => sum + (bucket.upload ?? 0), 0)).toBe(
      37,
    )
  })
  it('shows a calendar month ending on the selected day', () => {
    const buckets = chartBuckets('2026-03-31', 30, [
      { period: '2026-03-31', download: 4, upload: 1 },
    ])
    expect(buckets).toHaveLength(31)
    expect(buckets[0].key).toBe('2026-03-01')
    expect(buckets.at(-1)?.download).toBe(4)
  })
  it('zooms to recent minute data without relabeling the end as midnight or truncating a legacy hour', () => {
    const full = chartBuckets('2026-10-30', 1, [
      { period: '2026-10-30T10:01', upload: 1, download: 2 },
      { period: '2026-10-30T10:02', upload: 2, download: 3 },
    ])
    const zoom = windowChartBuckets(full, 60)
    expect(zoom).toHaveLength(60)
    expect(zoom[0].key).toBe('2026-10-30T09:03')
    expect(timeAxisTicks(zoom, 1, 1000).at(-1)?.label).not.toBe('24:00')
    const mixed = chartBuckets('2026-10-30', 1, [
      { period: '2026-10-30T10', upload: 100, download: 200 },
      { period: '2026-10-30T11:30', upload: 1, download: 2 },
    ])
    const legacyZoom = windowChartBuckets(mixed, 60)
    expect(legacyZoom[0].span).toBe(1)
    expect(
      legacyZoom.reduce((sum, value) => sum + (value.upload ?? 0), 0),
    ).toBe(1)
  })
})

describe('traffic curve', () => {
  it('stacks upload first and places the curve point at the combined bar top', () => {
    const { maximum, bars } = stackedBars(
      [{ key: 'hour', label: 'hour', upload: 30, download: 70 }],
      { left: 0, right: 100, top: 0, bottom: 100 },
    )
    expect(maximum).toBe(100)
    expect(bars[0]).toMatchObject({ x: 50, top: 0, split: 70, bottom: 100 })
  })

  it('joins adjacent recorded bars while leaving missing periods unrecorded', () => {
    const buckets = chartBuckets('2026-10-30', 1, [
      { period: '2026-10-30T10:01', upload: 3, download: 7 },
      { period: '2026-10-30T10:02', upload: 4, download: 5 },
    ])
    const bars = stackedBars(buckets, {
      left: 0,
      right: 1440,
      top: 0,
      bottom: 100,
    }).bars
    expect(bars[600]).toBeNull()
    expect(bars[601]!.x + bars[601]!.width / 2).toBeGreaterThanOrEqual(
      bars[602]!.x - bars[602]!.width / 2,
    )
    expect(bars[603]).toBeNull()
    const areas = stackedAreas(buckets, bars, 1)
    expect(areas).toHaveLength(1)
    expect(areas[0].upload).toMatch(/^M 601 /)
    expect(areas[0].upload).toContain('C ')
    expect(areas[0].upload).toContain('L 603 100')
    expect(areas[0].download.startsWith(areas[0].totalCurve)).toBe(true)
  })
})

describe('adaptive chart scales', () => {
  it('uses 15 minute tick marks over a day while keeping labels spaced', () => {
    const buckets = chartBuckets('2026-10-30', 1, [])
    const ticks = timeAxisTicks(buckets, 1, 1500)
    expect(ticks).toHaveLength(97)
    expect(ticks[1].minute).toBe(15)
    expect(ticks[0].label).toBe('00:00')
    expect(ticks.at(-1)?.label).toBe('24:00')
    expect(ticks.filter((tick) => tick.label).length).toBeLessThan(ticks.length)
  })
  it('adjusts labels and thins subdivisions in very narrow plots', () => {
    const buckets = chartBuckets('2026-10-30', 1, [])
    const narrow = timeAxisTicks(buckets, 1, 250)
    const wide = timeAxisTicks(buckets, 1, 1500)
    const huge = timeAxisTicks(buckets, 1, 5000)
    expect(narrow.length).toBeLessThan(wide.length)
    expect(huge.filter((tick) => tick.label).length).toBeGreaterThan(
      wide.filter((tick) => tick.label).length,
    )
    expect(valueAxisTicks(160)).toHaveLength(33)
    expect(valueAxisTicks(160).filter((tick) => tick.major)).toHaveLength(5)
    expect(valueAxisTicks(60).length).toBeLessThan(33)
  })
  it('uses the same eightfold subdivisions across longer ranges and a year boundary', () => {
    const threeDays = timeAxisTicks(chartBuckets('2026-01-02', 3, []), 3, 1800)
    expect(threeDays[1].minute).toBe(45)
    expect(threeDays[0].label).toBe('12-31 00:00')
    const week = timeAxisTicks(chartBuckets('2026-01-02', 7, []), 7, 1800)
    expect(week[1].minute).toBe(180)
    const month = timeAxisTicks(chartBuckets('2026-01-02', 30, []), 30, 1800)
    expect(month[1].minute).toBe(540)
  })
})
