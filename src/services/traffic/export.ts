import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { save } from '@tauri-apps/plugin-dialog'

import { mutate } from '@/services/mutate'

import { exportTrafficCsv, backupTrafficDatabase } from './api'

export const copyTrafficDiagnostics = (content: string) =>
  mutate(() => writeText(content), {
    id: 'copy-traffic-diagnostics',
    errorNotice: false,
  })

export async function saveTrafficDatabase(defaultPath: string) {
  const destination = await save({
    defaultPath,
    filters: [{ name: 'SQLite', extensions: ['sqlite3', 'db'] }],
  })
  if (!destination) return null
  await mutate(() => backupTrafficDatabase(destination), {
    id: 'traffic-history',
    errorNotice: false,
  })
  return destination
}

export async function saveTrafficCsv(content: string, filename: string) {
  const destination = await save({
    defaultPath: filename,
    filters: [{ name: 'CSV', extensions: ['csv'] }],
  })
  if (!destination) return null
  await mutate(() => exportTrafficCsv(destination, content), {
    id: 'export-traffic-csv',
    errorNotice: false,
  })
  return destination
}
