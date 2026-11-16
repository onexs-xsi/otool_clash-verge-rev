import { expect, it } from 'vitest'

import { searchSettings } from '@/components/setting/settings-catalog'

it('finds the automatic release-check preference for the portable fork', () => {
  const results = searchSettings(
    'auto check update',
    'windows',
    (key) => key,
    (key) => key,
  )
  expect(results.map((entry) => entry.id)).toContain(
    'settings.modals.misc.fields.autoCheckUpdate',
  )
})
