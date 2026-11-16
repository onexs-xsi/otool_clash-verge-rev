export type TrafficScaleMode = 'adaptive' | 'linear' | 'log'

export function trafficValueScale(values: number[], mode: TrafficScaleMode) {
  const positive = values
    .filter((value) => value > 0 && Number.isFinite(value))
    .sort((a, b) => a - b)
  const maximum = Math.max(1, ...positive)
  const median = positive[Math.floor((positive.length - 1) / 2)] ?? 1
  const kind =
    mode === 'linear'
      ? 'linear'
      : mode === 'log' || maximum / median >= 16
        ? 'log'
        : 'sqrt'
  const knee = Math.max(1, median / 4)
  const logarithm = Math.log1p(maximum / knee)
  const forward = (value: number) => {
    const fraction = Math.max(0, Math.min(maximum, value)) / maximum
    if (kind === 'linear') return fraction
    return kind === 'sqrt'
      ? Math.sqrt(fraction)
      : Math.log1p((fraction * maximum) / knee) / logarithm
  }
  const inverse = (fraction: number) => {
    if (kind === 'linear') return maximum * fraction
    return kind === 'sqrt'
      ? maximum * fraction ** 2
      : knee * Math.expm1(fraction * logarithm)
  }
  return { maximum, kind, forward, inverse }
}
