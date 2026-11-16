import { describe, expect, it } from 'vitest'

import { trafficRoute } from './traffic-metrics'

describe('final outbound accounting', () => {
  it('uses the final outbound rather than a group name containing Direct', () => {
    expect(trafficRoute('Bilibili / 🎯Direct / DIRECT')).toBe('direct')
    expect(trafficRoute('Direct / Japan')).toBe('proxy')
    expect(trafficRoute('DIRECT / Japan')).toBe('proxy')
    expect(trafficRoute('REJECT')).toBe('rejected')
    expect(trafficRoute('')).toBe('unknown')
  })
})
