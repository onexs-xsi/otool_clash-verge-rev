export interface BandPoint {
  x: number
  totalY: number
  uploadY: number
}

export interface BezierSegment {
  from: { x: number; y: number }
  c1: { x: number; y: number }
  c2: { x: number; y: number }
  to: { x: number; y: number }
}

const tangents = (points: BandPoint[], field: 'totalY' | 'uploadY') => {
  const slopes = points
    .slice(1)
    .map(
      (point, i) => (point[field] - points[i][field]) / (point.x - points[i].x),
    )
  return points.map((_, i) => {
    if (i === 0) return slopes[0] ?? 0
    if (i === points.length - 1) return slopes.at(-1) ?? 0
    const a = slopes[i - 1]
    const b = slopes[i]
    return a * b <= 0 ? 0 : (2 * a * b) / (a + b)
  })
}

export function fitStackedBand(
  points: BandPoint[],
  top: number,
  bottom: number,
) {
  const total = tangents(points, 'totalY')
  const upload = tangents(points, 'uploadY')
  for (let i = 0; i < points.length; i += 1) {
    let factor = 1
    const constrain = (gap: number, change: number) => {
      if (change < 0) factor = Math.min(factor, Math.max(0, gap) / -change)
    }
    const steps = [
      i > 0 ? -(points[i].x - points[i - 1].x) / 3 : 0,
      i + 1 < points.length ? (points[i + 1].x - points[i].x) / 3 : 0,
    ]
    for (const step of steps) {
      constrain(
        points[i].uploadY - points[i].totalY,
        step * (upload[i] - total[i]),
      )
      constrain(bottom - points[i].uploadY, -step * upload[i])
      constrain(points[i].totalY - top, step * total[i])
    }
    // Shared derivative scaling keeps both curves C1-continuous and ordered.
    total[i] *= factor
    upload[i] *= factor
  }
  const segments = (
    field: 'totalY' | 'uploadY',
    derivatives: number[],
  ): BezierSegment[] =>
    points.slice(1).map((point, i) => {
      const previous = points[i]
      const dx = (point.x - previous.x) / 3
      return {
        from: { x: previous.x, y: previous[field] },
        c1: { x: previous.x + dx, y: previous[field] + derivatives[i] * dx },
        c2: { x: point.x - dx, y: point[field] - derivatives[i + 1] * dx },
        to: { x: point.x, y: point[field] },
      }
    })
  const upper = segments('totalY', total)
  const lower = segments('uploadY', upload)
  const commands = (values: BezierSegment[], reverse = false) =>
    (reverse ? values.slice().reverse() : values)
      .map((segment) => {
        const first = reverse ? segment.c2 : segment.c1
        const second = reverse ? segment.c1 : segment.c2
        const end = reverse ? segment.from : segment.to
        return `C ${first.x} ${first.y} ${second.x} ${second.y} ${end.x} ${end.y}`
      })
      .join(' ')
  const first = points[0]
  const last = points.at(-1)!
  const totalCurve = `M ${first.x} ${first.totalY} ${commands(upper)}`
  const uploadCurve = `M ${first.x} ${first.uploadY} ${commands(lower)}`
  return {
    upper,
    lower,
    totalCurve,
    uploadCurve,
    upload: `${uploadCurve} L ${last.x} ${bottom} L ${first.x} ${bottom} Z`,
    download: `${totalCurve} L ${last.x} ${last.uploadY} ${commands(lower, true)} Z`,
  }
}
