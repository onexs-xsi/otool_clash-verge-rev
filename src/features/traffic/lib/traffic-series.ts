import dayjs from 'dayjs'

import type { TrafficSeriesPoint } from '@/services/traffic/api'

import { fitStackedBand } from './traffic-area'
import { trafficValueScale, type TrafficScaleMode } from './traffic-scale'

export type ChartRange = 1 | 3 | 7 | 30
export type ChartWindow = ChartRange | '1h' | '6h'
export interface ChartBucket {
  key: string
  label: string
  download: number | null
  upload: number | null
  span?: number
  legacyHour?: boolean
}

export function windowChartBuckets(
  buckets: ChartBucket[],
  minutes: 60 | 360,
  fallbackEnd = 1440,
) {
  const factor = buckets[0]?.key.length === 19 ? 12 : 1
  const width = minutes * factor
  let last = 0
  for (let index = 0; index < buckets.length; index += 1) {
    const bucket = buckets[index]
    if (bucket.upload != null && bucket.download != null)
      last = index + (bucket.span ?? 1)
  }
  const end = Math.min(
    buckets.length,
    Math.max(width, last || fallbackEnd * factor),
  )
  let start = Math.max(0, end - width)
  if (buckets[start]?.span === 0) start -= start % (60 * factor)
  return buckets.slice(start, end)
}

export function chartBuckets(
  day: string,
  range: ChartRange,
  history: TrafficSeriesPoint[],
): ChartBucket[] {
  if (range === 1) {
    const fine = history.some((point) => point.period.length === 19)
    const seconds =
      fine || !history.some((point) => point.period.length === 16) ? 5 : 60
    const values = new Map(
      history
        .filter((point) => point.period.length > 13)
        .map((point) => [point.period, point]),
    )
    const pad = (value: number) => String(value).padStart(2, '0')
    return Array.from({ length: 86400 / seconds }, (_, index) => {
      const elapsed = index * seconds
      const label = `${pad(Math.floor(elapsed / 3600))}:${pad(Math.floor(elapsed / 60) % 60)}${seconds === 5 ? `:${pad(elapsed % 60)}` : ''}`
      const key = `${day}T${label}`
      const point = values.get(key)
      return {
        key,
        label,
        span: 1,
        legacyHour: false,
        upload: point?.upload ?? null,
        download: point?.download ?? null,
      }
    })
  }
  const end = dayjs(`${day}T00:00:00`)
  const start =
    range === 30
      ? end.subtract(1, 'month').add(1, 'day')
      : end.subtract(range - 1, 'day')
  const byPeriod = new Map(history.map((point) => [point.period, point]))
  const hourly = range === 3
  const length = hourly ? 24 : end.diff(start, 'day') + 1
  return Array.from({ length }, (_, index) => {
    const time = hourly ? start.add(index * 3, 'hour') : start.add(index, 'day')
    const key = time.format(hourly ? 'YYYY-MM-DDTHH' : 'YYYY-MM-DD')
    const values = (
      hourly
        ? Array.from({ length: 3 }, (_, offset) =>
            byPeriod.get(time.add(offset, 'hour').format('YYYY-MM-DDTHH')),
          )
        : [byPeriod.get(key)]
    ).filter((value): value is TrafficSeriesPoint => value != null)
    return {
      key,
      label: time.format(hourly ? 'MM-DD HH:mm' : 'MM-DD'),
      span: 1,
      download: values.length
        ? values.reduce((sum, value) => sum + value.download, 0)
        : null,
      upload: values.length
        ? values.reduce((sum, value) => sum + value.upload, 0)
        : null,
    }
  })
}

export interface StackedBar {
  x: number
  width: number
  top: number
  split: number
  bottom: number
}

export interface TimeAxisTick {
  minute: number
  fraction: number
  label: string | null
}

/** Subdivide the previous major interval eightfold, thinning marks and labels only when space requires it. */
export function timeAxisTicks(
  buckets: ChartBucket[],
  range: ChartRange,
  plotWidth: number,
): TimeAxisTick[] {
  if (!buckets.length || plotWidth <= 0) return []
  const minutesPerBucket =
    range === 1
      ? buckets[0].key.length === 19
        ? 1 / 12
        : 1
      : range === 3
        ? 180
        : 1440
  const totalMinutes = buckets.length * minutesPerBucket
  const oldMajorBuckets = Math.max(1, Math.ceil(buckets.length / 12))
  const requestedMinor = (oldMajorBuckets * minutesPerBucket) / 8
  const requestedMinorMinutes =
    range === 1
      ? ([1, 2, 5, 10, 15, 30, 60].find((value) => value >= requestedMinor) ??
        requestedMinor)
      : requestedMinor
  let minuteStep = requestedMinorMinutes
  while ((minuteStep / totalMinutes) * plotWidth < 4) minuteStep *= 2

  const labelWidth = range === 1 ? 50 : 92
  let labelEvery = 1
  while ((minuteStep * labelEvery * plotWidth) / totalMinutes < labelWidth)
    labelEvery *= 2

  const startKey = buckets[0].key
  const startText =
    startKey.length === 10
      ? `${startKey}T00:00:00Z`
      : startKey.length === 13
        ? `${startKey}:00:00Z`
        : startKey.length === 19
          ? `${startKey}Z`
          : `${startKey}:00Z`
  const start = new Date(startText).getTime()
  const format = (minutes: number) => {
    const date = new Date(start + minutes * 60_000)
    if (
      range === 1 &&
      minutes > 0 &&
      date.getUTCHours() === 0 &&
      date.getUTCMinutes() === 0
    )
      return '24:00'
    const pad = (value: number) => String(value).padStart(2, '0')
    const time = `${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())}`
    if (range === 1) return time
    const dateLabel = `${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`
    return minuteStep * labelEvery < 1440 ? `${dateLabel} ${time}` : dateLabel
  }
  const count = Math.floor(totalMinutes / minuteStep)
  const ticks = Array.from({ length: count + 1 }, (_, index) => {
    const minute = index * minuteStep
    return {
      minute,
      fraction: minute / totalMinutes,
      label: index % labelEvery === 0 ? format(minute) : null,
    }
  })
  const last = ticks.at(-1)!
  if (last.minute === totalMinutes && last.label == null) {
    const previous = ticks
      .slice(0, -1)
      .reverse()
      .find((tick) => tick.label != null)
    if (
      previous &&
      (last.fraction - previous.fraction) * plotWidth < labelWidth
    )
      previous.label = null
    last.label = format(last.minute)
  }
  return ticks
}

/** Preserve eight minor intervals per original major interval without crowding short charts. */
export function valueAxisTicks(plotHeight: number) {
  let factor = 1
  while ((plotHeight * factor) / 32 < 4 && factor < 8) factor *= 2
  const intervals = 32 / factor
  return Array.from({ length: intervals + 1 }, (_, index) => ({
    fraction: index / intervals,
    major: (index * factor) % 8 === 0,
  }))
}

/** Stack upload at the baseline and download above it, using the combined scale. */
export function stackedBars(
  buckets: ChartBucket[],
  bounds: { left: number; right: number; top: number; bottom: number },
  scaleMode: TrafficScaleMode = 'linear',
) {
  const scale = trafficValueScale(
    buckets.map((bucket) => (bucket.upload ?? 0) + (bucket.download ?? 0)),
    scaleMode,
  )
  const maximum = scale.maximum
  const step = (bounds.right - bounds.left) / Math.max(1, buckets.length)
  const height = bounds.bottom - bounds.top
  const bars = buckets.map((bucket, index): StackedBar | null => {
    if (bucket.upload == null || bucket.download == null) return null
    const span = bucket.span ?? 1
    const total = bucket.upload + bucket.download
    const totalHeight = scale.forward(total) * height
    return {
      x: bounds.left + step * (index + span / 2),
      width: step * span,
      top: bounds.bottom - totalHeight,
      split:
        bounds.bottom - (total > 0 ? bucket.upload / total : 0) * totalHeight,
      bottom: bounds.bottom,
    }
  })
  return { maximum, step, bars, scale }
}

/** Fit both boundaries and use the same curves for their strokes and area fills. */
export function stackedAreas(
  buckets: ChartBucket[],
  bars: (StackedBar | null)[],
  step: number,
) {
  const runs: { bar: StackedBar; span: number }[][] = []
  let run: { bar: StackedBar; span: number }[] = []
  for (let index = 0; index < bars.length; index += 1) {
    const span = buckets[index].span ?? 1
    if (span === 0) continue
    const bar = bars[index]
    if (bar) run.push({ bar, span })
    else if (run.length) {
      runs.push(run)
      run = []
    }
  }
  if (run.length) runs.push(run)
  return runs.map((items) => {
    let points = items.map(({ bar }) => ({
      x: bar.x,
      totalY: bar.top,
      uploadY: bar.split,
    }))
    if (step < 0.5 && points.length > 16) {
      const selected = new Set<number>([0, points.length - 1])
      for (let first = 0; first < points.length; ) {
        let last = first + 1
        const pixel = Math.floor(points[first].x)
        while (last < points.length && Math.floor(points[last].x) === pixel)
          last += 1
        selected.add(first)
        selected.add(last - 1)
        for (const field of ['totalY', 'uploadY', 'band'] as const) {
          const value = (index: number) =>
            field === 'band'
              ? points[index].uploadY - points[index].totalY
              : points[index][field]
          let minimum = first
          let maximum = first
          for (let index = first + 1; index < last; index += 1) {
            if (value(index) < value(minimum)) minimum = index
            if (value(index) > value(maximum)) maximum = index
          }
          selected.add(minimum)
          selected.add(maximum)
        }
        first = last
      }
      // Only subpixel vertices are reduced; both boundaries keep their local extrema.
      points = [...selected].sort((a, b) => a - b).map((index) => points[index])
    }
    const first = items[0]
    const last = items.at(-1)!
    points.unshift({ ...points[0], x: first.bar.x - (step * first.span) / 2 })
    points.push({ ...points.at(-1)!, x: last.bar.x + (step * last.span) / 2 })
    return {
      start: first.bar.x,
      ...fitStackedBand(
        points,
        Math.min(...points.map((point) => point.totalY)),
        first.bar.bottom,
      ),
    }
  })
}
