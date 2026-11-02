import { describe, expect, it } from 'vitest'

import { resolveMenuOrder } from './use-nav-menu-order'

const items = ['/', '/connections', '/traffic', '/rules'].map((path) => ({
  path,
}))
const defaults = items.map((item) => item.path)
const lookup = new Map(items.map((item) => [item.path, item]))

describe('resolveMenuOrder', () => {
  it('inserts the new traffic page after connections while preserving the saved order', () => {
    expect(
      resolveMenuOrder(['/rules', '/connections', '/'], defaults, lookup),
    ).toEqual(['/rules', '/connections', '/traffic', '/'])
  })

  it('preserves an existing placement chosen by the user', () => {
    expect(
      resolveMenuOrder(
        ['/traffic', '/rules', '/connections', '/'],
        defaults,
        lookup,
      ),
    ).toEqual(['/traffic', '/rules', '/connections', '/'])
  })
})
