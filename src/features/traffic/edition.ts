import edition from '@root/traffic.edition.json'

export const automaticApplicationUpdates = edition.automaticApplicationUpdates
export const trafficEdition = import.meta.env.VITE_TRAFFIC_EDITION || ''
export const trafficDisplayName = edition.displayName

const repository = import.meta.env.VITE_TRAFFIC_REPOSITORY || edition.repository
export const trafficReleasesUrl =
  typeof repository === 'string' &&
  /^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/?$/.test(repository)
    ? `${repository.replace(/\/$/, '')}/releases`
    : null
