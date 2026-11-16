import edition from '@root/traffic.edition.json'

export const automaticApplicationUpdates = edition.automaticApplicationUpdates
export const trafficDisplayName = edition.displayName
export const trafficUpstreamVersion = edition.upstream.version

const repository = import.meta.env.VITE_TRAFFIC_REPOSITORY || edition.repository
export const trafficRepositoryUrl =
  typeof repository === 'string' &&
  /^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/?$/.test(repository)
    ? repository.replace(/\/$/, '')
    : null
export const trafficReleasesUrl = trafficRepositoryUrl
  ? `${trafficRepositoryUrl}/releases`
  : null
