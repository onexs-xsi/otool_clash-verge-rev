import { commands } from '@/services/bindings'
import type {
  ApplicationProcess,
  TrafficDiagnosticView,
  TrafficRow_Serialize as TrafficRow,
  TrafficView_Serialize as TrafficView,
  WslTrafficStatus as NativeWslTrafficStatus,
} from '@/services/bindings'

export type {
  DailyTrafficPoint,
  TrafficSeriesPoint,
  StorageInfo as TrafficStorageInfo,
} from '@/services/bindings'

export type TrafficStatisticsRow = Omit<
  TrafficRow,
  'active' | 'lastActivityAt' | 'uploadRate' | 'downloadRate'
> & {
  active?: boolean
  lastActivityAt?: number | null
  uploadRate: number
  downloadRate: number
}

export type TrafficStatisticsView = Omit<
  TrafficView,
  'rows' | 'historyBytes'
> & {
  historyBytes?: number | null
  rows: TrafficStatisticsRow[]
}

export async function getTrafficStatistics(
  day: string,
  month: string,
): Promise<TrafficStatisticsView> {
  const view = await commands.getTrafficStatistics(day, month)
  return {
    ...view,
    rows: view.rows.map((row) => ({
      ...row,
      uploadRate: row.uploadRate ?? 0,
      downloadRate: row.downloadRate ?? 0,
    })),
  }
}

export async function clearTrafficHistory(day: string | null) {
  return commands.clearTrafficHistory(day)
}

export async function exportTrafficCsv(destination: string, content: string) {
  return commands.exportTrafficCsv(destination, content)
}

export type WslTrafficStatus = Omit<NativeWslTrafficStatus, 'phase'> & {
  phase: 'disabled' | 'monitoring' | 'tun_off' | 'waiting'
}

export async function getWslTrafficStatus() {
  return commands.getWslTrafficStatus() as Promise<WslTrafficStatus>
}

export type TrafficApplicationProcess = Omit<
  ApplicationProcess,
  'matchSource'
> & {
  matchSource:
    | 'path'
    | 'name'
    | 'socket'
    | 'installation-family'
    | 'tun-local-socket'
}

export type TrafficDiagnostic = Omit<
  TrafficDiagnosticView,
  'reason' | 'connectionActive' | 'process' | 'liveProcess'
> & {
  reason: 'wsl_process_unresolved' | 'missing_endpoints' | 'process_unresolved'
  connectionActive?: boolean | null
  process?: TrafficApplicationProcess | null
  liveProcess?: TrafficApplicationProcess | null
}

export async function getTrafficApplicationProcesses(
  app: string,
  path: string,
) {
  return commands.getTrafficApplicationProcesses(app, path) as Promise<
    TrafficApplicationProcess[]
  >
}

export async function backupTrafficDatabase(destination: string) {
  return commands.backupTrafficDatabase(destination)
}

export const getTrafficStorageInfo = (checkIntegrity = false) =>
  commands.getTrafficStorageInfo(checkIntegrity)

export const compactTrafficDatabase = () => commands.compactTrafficDatabase()

export async function getTrafficApplicationDiagnostics(
  app: string,
  path: string,
  day: string,
  chains: string[],
) {
  return commands.getTrafficApplicationDiagnostics(
    app,
    path,
    day,
    chains,
  ) as Promise<TrafficDiagnostic[]>
}

export async function getTrafficApplicationSeries(
  app: string,
  path: string,
  day: string,
  days: number,
  chains: string[] = [],
  proxyOnly = true,
) {
  return commands.getTrafficApplicationSeries(
    app,
    path,
    day,
    days,
    chains,
    proxyOnly,
  )
}

export async function getTrafficAppIcon(path: string, app = '') {
  return commands.getTrafficAppIcon(path, app)
}
