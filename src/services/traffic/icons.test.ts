import { describe, expect, it, vi } from 'vitest'

import { createTrafficIconLoader } from './icons'

describe('traffic icon loading', () => {
  it('coalesces matching requests and retries a failed cached extraction after expiration', async () => {
    let now = 0
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(null)
      .mockResolvedValue('data:image/png;base64,icon')
    const loader = createTrafficIconLoader(fetch, () => now)
    await Promise.all([
      loader.get('C:\\App.exe', 'App.exe'),
      loader.get('c:/app.exe', 'app.exe'),
    ])
    expect(fetch).toHaveBeenCalledTimes(1)
    await loader.get('C:\\App.exe', 'App.exe')
    expect(fetch).toHaveBeenCalledTimes(1)
    now = 60_001
    expect(await loader.get('C:\\App.exe', 'App.exe')).toContain(
      'data:image/png',
    )
    expect(fetch).toHaveBeenCalledTimes(2)
  })
  it('limits concurrent native extraction to four requests', async () => {
    let active = 0
    let maximum = 0
    const loader = createTrafficIconLoader(async () => {
      active += 1
      maximum = Math.max(maximum, active)
      await new Promise((resolve) => setTimeout(resolve, 1))
      active -= 1
      return null
    })
    await Promise.all(
      Array.from({ length: 20 }, (_, i) => loader.get(`${i}.exe`, '')),
    )
    expect(maximum).toBe(4)
  })
})
