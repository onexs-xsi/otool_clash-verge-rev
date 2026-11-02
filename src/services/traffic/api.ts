import { invoke } from '@tauri-apps/api/core'

export interface TrafficStatisticsRow {
  app: string
  path: string
  chain: string
  active?: boolean
  lastActivityAt?: number | null
  dayDurationMs: number | null
  durationIncomplete: boolean
  uploadRate: number
  downloadRate: number
  dayUpload: number
  dayDownload: number
  monthUpload: number
  monthDownload: number
}

export interface TrafficStatisticsView {
  connected: boolean
  loaded: boolean
  historyError: string | null
  historyPath: string | null
  historyBytes?: number | null
  rows: TrafficStatisticsRow[]
}

export async function getTrafficStatistics(day: string, month: string) {
  return invoke<TrafficStatisticsView>('get_traffic_statistics', { day, month })
}

export async function clearTrafficHistory(day: string | null) {
  return invoke<void>('clear_traffic_history', { day })
}

export async function exportTrafficCsv(destination: string, content: string) {
  return invoke<void>('export_traffic_csv', { destination, content })
}

export interface WslTrafficStatus {
  enabled: boolean
  phase: 'disabled' | 'monitoring' | 'tun_off' | 'waiting'
  distributions: string[]
  distro: string | null
  directDetectionAvailable: boolean
  natMappingAvailable: boolean
  error: string | null
}

export async function getWslTrafficStatus() {
  return invoke<WslTrafficStatus>('get_wsl_traffic_status')
}

export interface DailyTrafficPoint {
  day: string
  upload: number
  download: number
}

export interface TrafficSeriesPoint {
  period: string
  upload: number
  download: number
}

export interface TrafficDiagnostic {
  chain: string
  destination: string
  source: string
  network: string
  reason: 'wsl_process_unresolved' | 'missing_endpoints' | 'process_unresolved'
  lastSeen: string
  upload: number
  download: number
  connectionId?: string
  inboundType?: string
  connectionActive?: boolean | null
  process?: TrafficApplicationProcess | null
  liveProcess?: TrafficApplicationProcess | null
}

export interface TrafficApplicationProcess {
  pid: number
  parentPid: number
  name: string
  path: string
  parentName: string
  matchSource:
    | 'path'
    | 'name'
    | 'socket'
    | 'installation-family'
    | 'tun-local-socket'
}

export async function getTrafficApplicationProcesses(
  app: string,
  path: string,
) {
  return invoke<TrafficApplicationProcess[]>(
    'get_traffic_application_processes',
    { app, path },
  )
}

export async function backupTrafficDatabase(destination: string) {
  return invoke<void>('backup_traffic_database', { destination })
}

export interface TrafficStorageInfo {
  path: string
  databaseBytes: number
  walBytes: number
  reusableBytes: number
  dailyRows: number
  sampleRows: number
  recordedDays: number
  firstDay: string | null
  lastDay: string | null
  lastSavedAt: number | null
  integrity: string | null
}

export const getTrafficStorageInfo = (checkIntegrity = false) =>
  invoke<TrafficStorageInfo>('get_traffic_storage_info', { checkIntegrity })

export const compactTrafficDatabase = () =>
  invoke<TrafficStorageInfo>('compact_traffic_database')

export async function getTrafficApplicationDiagnostics(
  app: string,
  path: string,
  day: string,
  chains: string[],
) {
  return invoke<TrafficDiagnostic[]>('get_traffic_application_diagnostics', {
    app,
    path,
    day,
    chains,
  })
}

export async function getTrafficApplicationSeries(
  app: string,
  path: string,
  day: string,
  days: number,
  chains: string[] = [],
  proxyOnly = true,
) {
  return invoke<TrafficSeriesPoint[]>('get_traffic_application_series', {
    app,
    path,
    day,
    days,
    chains,
    proxyOnly,
  })
}

export async function getTrafficApplicationHistory(
  app: string,
  path: string,
  month: string,
) {
  return invoke<DailyTrafficPoint[]>('get_traffic_application_history', {
    app,
    path,
    month,
  })
}

export async function getTrafficAppIcon(path: string, app = '') {
  return invoke<string | null>('get_traffic_app_icon', { path, app })
}
