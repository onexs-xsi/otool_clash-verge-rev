import { describe, expect, it } from 'vitest'

import { monthlySeries } from './monthly-series'

describe('monthlySeries', () => {
  it('keeps unrecorded days distinct from recorded traffic, including leap days', () => {
    const result = monthlySeries('2028-02', [
      { day: '2028-02-02', upload: 3, download: 7 },
      { day: '2028-02-29', upload: 0, download: 2 },
    ])
    expect(result).toHaveLength(29)
    expect(result[0].traffic).toBeNull()
    expect(result[1].traffic).toEqual({
      day: '2028-02-02',
      upload: 3,
      download: 7,
    })
    expect(result[28].traffic?.download).toBe(2)
  })
})
