import { expect, it } from 'vitest'

import { trafficValueScale } from './traffic-scale'
import { stackedBars } from './traffic-series'

it('expands small values while preserving zero and invertible original byte values', () => {
  const scale = trafficValueScale([100, 200, 400, 10000000], 'adaptive')
  expect(scale.kind).toBe('log')
  expect(scale.forward(100)).toBeGreaterThan(0.05)
  expect(scale.forward(0)).toBe(0)
  expect(scale.forward(10000000)).toBeCloseTo(1)
  for (const value of [0, 100, 10000, 10000000])
    expect(scale.inverse(scale.forward(value))).toBeCloseTo(value, 5)
})

it('keeps upload/download proportions inside nonlinear bars and allows a linear check', () => {
  const buckets = [
    { key: 'a', label: 'a', upload: 30, download: 70 },
    { key: 'b', label: 'b', upload: 0, download: 10000 },
  ]
  const bounds = { left: 0, right: 100, top: 0, bottom: 100 }
  const adaptive = stackedBars(buckets, bounds, 'adaptive').bars[0]!
  expect(
    (adaptive.bottom - adaptive.split) / (adaptive.bottom - adaptive.top),
  ).toBeCloseTo(0.3)
  expect(adaptive.top).toBeLessThan(99)
  expect(stackedBars(buckets, bounds, 'linear').bars[0]!.top).toBe(99)
})
