import dayjs from 'dayjs'
import { useEffect, useRef, useState } from 'react'

import { mutate } from '@/services/mutate'
import {
  clearTrafficHistory,
  getTrafficStatistics,
  type TrafficStatisticsView,
} from '@/services/traffic/api'

const localDay = () => dayjs().format('YYYY-MM-DD')

export function useTrafficHistory({
  day,
  month,
  onDayChange,
  onMonthChange,
  onResetPage,
}: {
  day: string
  month: string
  onDayChange: (day: string) => void
  onMonthChange: (month: string) => void
  onResetPage: (page: number) => void
}) {
  const [view, setView] = useState<TrafficStatisticsView | null>(null)
  const [failed, setFailed] = useState(false)
  const generationRef = useRef(0)
  const historyRevisionRef = useRef(0)
  const historyMutatingRef = useRef(false)
  const clockDayRef = useRef(localDay())

  useEffect(() => {
    generationRef.current += 1
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const refresh = async () => {
      try {
        const today = localDay()
        if (today !== clockDayRef.current) {
          const previous = clockDayRef.current
          clockDayRef.current = today
          if (day === previous) {
            onDayChange(today)
            if (month === previous.slice(0, 7)) onMonthChange(today.slice(0, 7))
            onResetPage(0)
            return
          }
        }
        if (document.hidden || historyMutatingRef.current) return
        const revision = historyRevisionRef.current
        const result = await getTrafficStatistics(day, month)
        if (active && revision === historyRevisionRef.current) {
          setView(result)
          setFailed(false)
        }
      } catch {
        if (active) setFailed(true)
      } finally {
        if (active) timer = setTimeout(refresh, 1000)
      }
    }
    void refresh()
    return () => {
      active = false
      generationRef.current += 1
      if (timer) clearTimeout(timer)
    }
  }, [day, month, onDayChange, onMonthChange, onResetPage])

  const clear = async (target: string | null) => {
    if (historyMutatingRef.current) return false
    const scope = generationRef.current
    historyMutatingRef.current = true
    historyRevisionRef.current += 1
    try {
      const result = await mutate(() => clearTrafficHistory(target), {
        id: 'traffic-history',
        errorNotice: false,
      })
      if (!result.ok) return false
      const refreshed = await getTrafficStatistics(day, month)
      if (generationRef.current === scope) {
        setView(refreshed)
        setFailed(false)
      }
      return true
    } finally {
      historyMutatingRef.current = false
    }
  }
  return { view, failed, clear }
}
