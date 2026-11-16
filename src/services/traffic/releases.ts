import { fetch } from '@tauri-apps/plugin-http'
import { compareVersions } from 'compare-versions'

import {
  trafficReleasesUrl,
  trafficRepositoryUrl,
} from '@/features/traffic/edition'
import { version as appVersion } from '@root/package.json'

interface GitHubRelease {
  draft: boolean
  tag_name: string
  body: string | null
  html_url: string
  assets: {
    name: string
    state: string
    browser_download_url: string
  }[]
}

export interface TrafficRelease {
  available: boolean
  version: string
  body: string
  url: string
  downloadUrl: string
}

export async function checkTrafficUpdate(): Promise<TrafficRelease | null> {
  if (!trafficRepositoryUrl || !trafficReleasesUrl) {
    throw new Error('The fork release repository is not configured.')
  }
  const repository = new URL(trafficRepositoryUrl).pathname
  const response = await fetch(
    `https://api.github.com/repos${repository}/releases?per_page=100`,
    {
      headers: {
        Accept: 'application/vnd.github+json',
        'X-GitHub-Api-Version': '2022-11-28',
        'User-Agent': `OTool-Clash-Verge/${appVersion}`,
      },
      connectTimeout: 10000,
      signal: AbortSignal.timeout(15000),
    },
  )
  if (!response.ok) {
    throw new Error(`GitHub release check failed (HTTP ${response.status}).`)
  }
  const releases: GitHubRelease[] = await response.json()
  if (!Array.isArray(releases)) {
    throw new Error('GitHub returned an invalid release list.')
  }
  const candidates: TrafficRelease[] = []
  for (const release of releases) {
    const match = /^v(\d+\.\d+\.\d+)-otool\.([1-9]\d*)$/.exec(release.tag_name)
    if (release.draft || !match) continue
    const url = `${trafficReleasesUrl}/tag/${release.tag_name}`
    if (release.html_url !== url) continue
    const assetName = `otool_clash-verge-rev_${match[1]}_traffic-v${match[2]}_windows-x64_portable.zip`
    const downloadUrl = `${trafficReleasesUrl}/download/${release.tag_name}/${assetName}`
    if (
      !release.assets.some(
        (asset) =>
          asset.state === 'uploaded' &&
          asset.name === assetName &&
          asset.browser_download_url === downloadUrl,
      )
    ) {
      continue
    }
    const version = release.tag_name.slice(1)
    candidates.push({
      available: compareVersions(version, appVersion) > 0,
      version,
      body: release.body ?? '',
      url,
      downloadUrl,
    })
  }
  candidates.sort((a, b) => compareVersions(b.version, a.version))
  return candidates[0] ?? null
}
