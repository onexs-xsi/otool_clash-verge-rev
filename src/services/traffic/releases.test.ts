import { beforeEach, describe, expect, it, vi } from 'vitest'

const { fetch } = vi.hoisted(() => ({ fetch: vi.fn() }))
vi.mock('@tauri-apps/plugin-http', () => ({ fetch }))
vi.mock('@root/package.json', () => ({ version: '2.6.0-otool.12' }))

import { checkTrafficUpdate } from './releases'

const repository = 'https://github.com/onexs-xsi/otool_clash-verge-rev'
const release = (tag: string, draft = false) => {
  const match = /^v(.+)-otool\.(\d+)$/.exec(tag)
  const asset = `otool_clash-verge-rev_${match?.[1]}_traffic-v${match?.[2]}_windows-x64_portable.zip`
  return {
    tag_name: tag,
    draft,
    prerelease: true,
    body: 'Release notes',
    html_url: `${repository}/releases/tag/${tag}`,
    assets: [
      {
        name: asset,
        state: 'uploaded',
        browser_download_url: `${repository}/releases/download/${tag}/${asset}`,
      },
    ],
  }
}
const respond = (releases: ReturnType<typeof release>[]) => {
  fetch.mockResolvedValue({ ok: true, json: async () => releases })
}

beforeEach(() => {
  fetch.mockReset()
})

describe('fork release checks', () => {
  it('checks the owner repository and compares numeric editions, including published prereleases', async () => {
    respond([
      release('v2.6.0-otool.9'),
      release('v2.6.0-otool.13'),
      release('v2.6.0-otool.12'),
      release('v2.6.0-otool.99', true),
      release('v3.0.0'),
    ])
    expect(await checkTrafficUpdate()).toMatchObject({
      available: true,
      version: '2.6.0-otool.13',
      body: 'Release notes',
      url: `${repository}/releases/tag/v2.6.0-otool.13`,
    })
    expect(fetch).toHaveBeenCalledWith(
      'https://api.github.com/repos/onexs-xsi/otool_clash-verge-rev/releases?per_page=100',
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    )
  })

  it('compares the upstream version before the edition number', async () => {
    respond([release('v2.6.0-otool.99'), release('v2.7.0-otool.1')])
    expect(await checkTrafficUpdate()).toMatchObject({
      available: true,
      version: '2.7.0-otool.1',
    })
  })

  it.each(['v2.6.0-otool.12', 'v2.6.0-otool.11'])(
    'does not offer an equal or older build: %s',
    async (tag) => {
      respond([release(tag)])
      expect(await checkTrafficUpdate()).toMatchObject({ available: false })
    },
  )

  it('does not offer drafts, incomplete packages or downloads outside the owner repository', async () => {
    const missing = { ...release('v2.6.0-otool.14'), assets: [] }
    const foreign = release('v2.6.0-otool.15')
    foreign.assets[0].browser_download_url = 'https://example.com/unrelated.zip'
    respond([release('v2.6.0-otool.99', true), missing, foreign])
    expect(await checkTrafficUpdate()).toBeNull()
  })

  it('distinguishes an empty repository from a failed check', async () => {
    respond([])
    expect(await checkTrafficUpdate()).toBeNull()
    fetch.mockResolvedValue({ ok: false, status: 403 })
    await expect(checkTrafficUpdate()).rejects.toThrow('HTTP 403')
    fetch.mockRejectedValue(new Error('Connection failed'))
    await expect(checkTrafficUpdate()).rejects.toThrow('Connection failed')
  })
})
