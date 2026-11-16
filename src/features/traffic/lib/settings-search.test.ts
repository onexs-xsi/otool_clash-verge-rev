import { expect, it } from 'vitest'

import { searchSettings } from '@/components/setting/settings-catalog'

it('excludes unsupported automatic application updates from settings search', () => {
  const results = searchSettings(
    'auto check update',
    'windows',
    (key) => key,
    (key) => key,
  )
  expect(results.map((entry) => entry.id)).not.toContain(
    'settings.modals.misc.fields.autoCheckUpdate',
  )
})
