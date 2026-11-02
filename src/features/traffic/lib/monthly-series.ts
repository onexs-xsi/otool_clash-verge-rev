import dayjs from 'dayjs'

import type { DailyTrafficPoint } from '@/services/traffic/api'

export function monthlySeries(month: string, history: DailyTrafficPoint[]) {
  const byDay = new Map(history.map((point) => [point.day, point]))
  return Array.from(
    { length: dayjs(`${month}-01`).daysInMonth() },
    (_, index) => {
      const day = `${month}-${String(index + 1).padStart(2, '0')}`
      return { day, traffic: byDay.get(day) ?? null }
    },
  )
}
