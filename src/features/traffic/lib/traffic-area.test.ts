import { expect, it } from 'vitest'

import { fitStackedBand, type BezierSegment } from './traffic-area'

const sample = (segment: BezierSegment, t: number) =>
  (1 - t) ** 3 * segment.from.y +
  3 * (1 - t) ** 2 * t * segment.c1.y +
  3 * (1 - t) * t ** 2 * segment.c2.y +
  t ** 3 * segment.to.y

it('fits both boundaries through samples and fills exactly to their curves without crossing or overshooting', () => {
  const points = [
    { x: 0, totalY: 80, uploadY: 90 },
    { x: 10, totalY: 20, uploadY: 25 },
    { x: 20, totalY: 25, uploadY: 99 },
    { x: 30, totalY: 90, uploadY: 92 },
    { x: 40, totalY: 30, uploadY: 40 },
    { x: 50, totalY: 10, uploadY: 95 },
    { x: 60, totalY: 100, uploadY: 100 },
  ]
  const band = fitStackedBand(points, 10, 100)
  for (let i = 0; i < band.upper.length; i += 1) {
    expect(band.upper[i].from.y).toBe(points[i].totalY)
    expect(band.lower[i].to.y).toBe(points[i + 1].uploadY)
    for (let step = 0; step <= 20; step += 1) {
      const upper = sample(band.upper[i], step / 20)
      const lower = sample(band.lower[i], step / 20)
      expect(upper).toBeGreaterThanOrEqual(10 - 1e-8)
      expect(lower).toBeGreaterThanOrEqual(upper - 1e-8)
      expect(lower).toBeLessThanOrEqual(100 + 1e-8)
    }
  }
  for (const segments of [band.upper, band.lower]) {
    for (let i = 1; i < segments.length; i += 1) {
      const a = segments[i - 1]
      const b = segments[i]
      expect((a.to.y - a.c2.y) / (a.to.x - a.c2.x)).toBeCloseTo(
        (b.c1.y - b.from.y) / (b.c1.x - b.from.x),
        6,
      )
    }
  }
  expect(band.upload.startsWith(band.uploadCurve)).toBe(true)
  expect(band.download.startsWith(band.totalCurve)).toBe(true)
  expect(band.download).toMatch(/ Z$/)
})
