import type { TrafficSortField } from './traffic-order'

export const trafficColumnWidths: Record<TrafficSortField, number> = {
  app: 224,
  chain: 208,
  dayDurationMs: 120,
  downloadRate: 160,
  uploadRate: 160,
  dayDownload: 152,
  dayUpload: 152,
  dayTotal: 152,
  monthDownload: 152,
  monthUpload: 152,
}
