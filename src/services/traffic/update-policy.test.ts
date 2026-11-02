import { describe, expect, it, vi } from 'vitest'

const { check } = vi.hoisted(() => ({ check: vi.fn() }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check }))

import { checkUpdateSafe } from '@/services/update'

describe('traffic edition application updates', () => {
  it('never queries the upstream installer feed for a portable fork', async () => {
    expect(await checkUpdateSafe()).toBeNull()
    expect(check).not.toHaveBeenCalled()
  })
})
