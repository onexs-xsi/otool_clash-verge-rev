import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined'
import RefreshRoundedIcon from '@mui/icons-material/RefreshRounded'
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  FormControlLabel,
  IconButton,
  MenuItem,
  Select as MuiSelect,
  Switch,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from '@mui/material'
import { useTheme } from '@mui/material/styles'
import dayjs from 'dayjs'
import { memo, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { formatConnectionTraffic } from '@/components/connection/connection-row-view'
import {
  getTrafficApplicationSeries,
  type TrafficSeriesPoint,
} from '@/services/traffic/api'

import type { TrafficScaleMode } from '../lib/traffic-scale'
import {
  chartBuckets,
  stackedAreas,
  stackedBars,
  timeAxisTicks,
  valueAxisTicks,
  windowChartBuckets,
  type ChartRange,
  type ChartWindow,
} from '../lib/traffic-series'

import { TrafficRecovery } from './traffic-recovery'

interface Props {
  app: string
  path: string
  day: string
  chains: string[]
}

const ranges: ChartWindow[] = ['1h', '6h', 1, 3, 7, 30]
const top = 18
const left = 84

export const TrafficChart = memo(function TrafficChart({
  app,
  path,
  day,
  chains,
}: Props) {
  const { t } = useTranslation()
  const theme = useTheme()
  const chartRef = useRef<HTMLDivElement>(null)
  const [chartWidth, setChartWidth] = useState(960)
  const [viewportHeight, setViewportHeight] = useState(() => window.innerHeight)
  const [selectedWindow, setSelectedWindow] = useState<ChartWindow>(1)
  const range: ChartRange =
    typeof selectedWindow === 'string' ? 1 : selectedWindow
  const [includeDirect, setIncludeDirect] = useState(false)
  const [scaleMode, setScaleMode] = useState<TrafficScaleMode>('adaptive')
  const [hoveredKey, setHoveredKey] = useState<string | null>(null)
  const [loaded, setLoaded] = useState<{
    key: string
    points: TrafficSeriesPoint[]
  } | null>(null)
  const [failedKey, setFailedKey] = useState('')
  const [refreshNonce, setRefreshNonce] = useState(0)
  const key = JSON.stringify([app, path, day, range, chains, includeDirect])
  const history = loaded?.key === key ? loaded.points : null
  const recovery = useMemo(
    () =>
      range === 1
        ? (history ?? []).filter(
            (point) =>
              point.period.length === 13 && point.upload + point.download > 0,
          )
        : [],
    [range, history],
  )
  const fullBuckets = useMemo(
    () => (history?.length ? chartBuckets(day, range, history) : []),
    [day, range, history],
  )
  const fallbackEnd =
    day === dayjs().format('YYYY-MM-DD')
      ? dayjs().hour() * 60 + dayjs().minute() + 1
      : 1440
  const buckets = useMemo(
    () =>
      typeof selectedWindow === 'string'
        ? windowChartBuckets(
            fullBuckets,
            selectedWindow === '1h' ? 60 : 360,
            fallbackEnd,
          )
        : fullBuckets,
    [fullBuckets, selectedWindow, fallbackEnd],
  )
  const right = chartWidth - 16
  const chartHeight = Math.round(
    Math.min(270, Math.max(190, viewportHeight * 0.2)),
  )
  const bottom = chartHeight - 32
  const { step, bars, scale } = useMemo(
    () =>
      stackedBars(
        buckets,
        {
          left,
          right,
          top,
          bottom,
        },
        scaleMode,
      ),
    [buckets, right, bottom, scaleMode],
  )
  const areas = useMemo(
    () => stackedAreas(buckets, bars, step),
    [buckets, bars, step],
  )
  const bucketIndexes = useMemo(
    () => new Map(buckets.map((bucket, index) => [bucket.key, index])),
    [buckets],
  )
  const hoveredIndex =
    hoveredKey == null ? -1 : (bucketIndexes.get(hoveredKey) ?? -1)
  const hovered = buckets[hoveredIndex]
  const hoveredBar = bars[hoveredIndex]
  const hoverX = hoveredBar?.x ?? left + step * (hoveredIndex + 0.5)
  const downloadLabel = t('traffic.download')
  const uploadLabel = t('traffic.upload')
  const totalLabel = t('traffic.chartTotal')
  const label = t('traffic.trendTitle')
  const noRecord = t('traffic.noRecord')

  useEffect(() => {
    const element = chartRef.current
    if (!element) return
    const observer = new ResizeObserver((entries) => {
      const width = Math.max(
        560,
        Math.round(entries[0]?.contentRect.width ?? 960),
      )
      setChartWidth((current) => (current === width ? current : width))
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    const resize = () => setViewportHeight(window.innerHeight)
    window.addEventListener('resize', resize)
    return () => window.removeEventListener('resize', resize)
  }, [])

  useEffect(() => {
    let active = true
    let pending = false
    let timer: ReturnType<typeof setTimeout> | undefined
    const refresh = async () => {
      if (!active || pending) return
      pending = true
      try {
        if (document.hidden) return
        const points = await getTrafficApplicationSeries(
          app,
          path,
          day,
          range,
          chains,
          !includeDirect,
        )
        if (active) {
          setLoaded({ key, points })
          setFailedKey('')
        }
      } catch {
        if (active) setFailedKey(key)
      } finally {
        pending = false
        if (active && day >= dayjs().format('YYYY-MM-DD'))
          timer = setTimeout(refresh, 5000)
      }
    }
    const visibility = () => {
      if (!document.hidden) {
        if (timer) clearTimeout(timer)
        void refresh()
      }
    }
    document.addEventListener('visibilitychange', visibility)
    void refresh()
    return () => {
      active = false
      if (timer) clearTimeout(timer)
      document.removeEventListener('visibilitychange', visibility)
    }
  }, [app, path, day, range, chains, includeDirect, key, refreshNonce])

  const timeTicks = timeAxisTicks(buckets, range, right - left)
  const valueTicks = valueAxisTicks(bottom - top)

  return (
    <Box sx={{ bgcolor: 'background.paper', borderRadius: 1, p: 1.5, mb: 1.5 }}>
      <Box
        sx={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          flexWrap: 'wrap',
          gap: 1,
        }}
      >
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            flexWrap: 'wrap',
            gap: 1.5,
          }}
        >
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
            {label}
          </Typography>
          {[
            { label: uploadLabel, color: theme.palette.secondary.main },
            { label: downloadLabel, color: theme.palette.primary.main },
          ].map((item) => (
            <Box
              key={item.label}
              sx={{ display: 'flex', alignItems: 'center', gap: 0.6 }}
            >
              <Box
                sx={{
                  width: 9,
                  height: 9,
                  borderRadius: '2px',
                  bgcolor: item.color,
                }}
              />
              <Typography variant="caption">{item.label}</Typography>
            </Box>
          ))}
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.6 }}>
            <svg aria-hidden="true" width="16" height="10">
              <line
                x1="0"
                y1="5"
                x2="16"
                y2="5"
                stroke={theme.palette.primary.main}
                strokeWidth="2"
              />
              <circle cx="8" cy="5" r="2" fill={theme.palette.primary.main} />
            </svg>
            <Typography variant="caption" color="text.secondary">
              {totalLabel}
            </Typography>
          </Box>
          <FormControlLabel
            sx={{ m: 0 }}
            control={
              <Switch
                size="small"
                checked={includeDirect}
                onChange={(_, value) => setIncludeDirect(value)}
              />
            }
            label={
              <Typography variant="caption">
                {t('traffic.includeDirect')}
              </Typography>
            }
          />
        </Box>
        <Box
          sx={{
            display: 'flex',
            gap: 0.75,
            alignItems: 'center',
            flexWrap: 'wrap',
          }}
        >
          <Tooltip
            title={t(
              failedKey === key ? 'traffic.chartStale' : 'traffic.chartRefresh',
            )}
          >
            <IconButton
              size="small"
              color={failedKey === key ? 'warning' : 'default'}
              aria-label={t('traffic.chartRefresh')}
              onClick={() => setRefreshNonce((value) => value + 1)}
            >
              <RefreshRoundedIcon fontSize="small" />
            </IconButton>
          </Tooltip>
          <MuiSelect
            size="small"
            value={scaleMode}
            inputProps={{
              'aria-label': t('traffic.scaleLabel'),
            }}
            onChange={(event) =>
              setScaleMode(event.target.value as TrafficScaleMode)
            }
            sx={{ fontSize: '0.8rem', width: 136, flexShrink: 0 }}
          >
            <MenuItem value="adaptive">{t('traffic.scaleAdaptive')}</MenuItem>
            <MenuItem value="log">{t('traffic.scaleLog')}</MenuItem>
            <MenuItem value="linear">{t('traffic.scaleLinear')}</MenuItem>
          </MuiSelect>
          <ToggleButtonGroup
            size="small"
            exclusive
            value={selectedWindow}
            aria-label={t('traffic.chartRangeLabel')}
            onChange={(_, value: ChartWindow | null) => {
              if (value != null) setSelectedWindow(value)
            }}
          >
            {ranges.map((value) => (
              <ToggleButton key={value} value={value}>
                {value === '1h'
                  ? t('traffic.rangeHour')
                  : value === '6h'
                    ? t('traffic.rangeSixHours')
                    : value === 1
                      ? t('traffic.rangeOne')
                      : value === 3
                        ? t('traffic.rangeThree')
                        : value === 7
                          ? t('traffic.rangeWeek')
                          : t('traffic.rangeMonth')}
              </ToggleButton>
            ))}
          </ToggleButtonGroup>
        </Box>
      </Box>
      <Box
        ref={chartRef}
        sx={{
          overflowX: 'auto',
          mt: 0.5,
          position: 'relative',
          minHeight: chartHeight + 8,
        }}
      >
        {failedKey === key && history === null ? (
          <Alert severity="error">{t('traffic.chartError')}</Alert>
        ) : history === null ? (
          <Box
            sx={{ height: chartHeight, display: 'grid', placeItems: 'center' }}
          >
            <CircularProgress size={24} />
          </Box>
        ) : buckets.every(
            (point) =>
              point.upload == null ||
              (point.upload === 0 && point.download === 0),
          ) ? (
          <Box
            sx={{
              height: chartHeight,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexDirection: 'column',
              gap: 1,
            }}
          >
            <Typography variant="body2" color="text.secondary">
              {recovery.length > 0
                ? t('traffic.recoveryChartEmpty')
                : includeDirect
                  ? t('traffic.trendEmpty')
                  : t('traffic.proxyChartEmpty')}
            </Typography>
            {!includeDirect && recovery.length === 0 && (
              <Button size="small" onClick={() => setIncludeDirect(true)}>
                {t('traffic.viewAllChart')}
              </Button>
            )}
          </Box>
        ) : (
          <svg
            data-value-scale={scale.kind}
            data-traffic-scope={includeDirect ? 'all' : 'proxy'}
            viewBox={`0 0 ${chartWidth} ${chartHeight}`}
            role="img"
            tabIndex={0}
            aria-label={`${label} · ${day} · ${t('traffic.chartKeyboardHint')}`}
            onPointerMove={(event) => {
              const rect = event.currentTarget.getBoundingClientRect()
              const x = ((event.clientX - rect.left) * chartWidth) / rect.width
              const y = ((event.clientY - rect.top) * chartHeight) / rect.height
              if (x < left || x > right || y < top || y > bottom) {
                setHoveredKey(null)
                return
              }
              let index = Math.min(
                buckets.length - 1,
                Math.max(0, Math.floor((x - left) / step)),
              )
              while (index > 0 && buckets[index].span === 0) index -= 1
              setHoveredKey(buckets[index].key)
            }}
            onPointerLeave={() => setHoveredKey(null)}
            onBlur={() => setHoveredKey(null)}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                setHoveredKey(null)
                return
              }
              if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft')
                return
              event.preventDefault()
              const direction = event.key === 'ArrowRight' ? 1 : -1
              let index =
                hoveredIndex >= 0
                  ? hoveredIndex + direction
                  : buckets.findIndex((bucket) => bucket.upload != null)
              index = Math.max(0, Math.min(buckets.length - 1, index))
              while (
                index > 0 &&
                index < buckets.length - 1 &&
                buckets[index].span === 0
              )
                index += direction
              while (index > 0 && buckets[index].span === 0) index -= 1
              setHoveredKey(buckets[index].key)
            }}
            style={{
              display: 'block',
              width: '100%',
              minWidth: chartWidth,
              height: chartHeight,
            }}
          >
            {valueTicks.map(({ fraction, major }) => {
              const y = bottom - fraction * (bottom - top)
              return (
                <g key={fraction}>
                  <line
                    data-value-tick="true"
                    data-grid-tick={major ? 'true' : undefined}
                    x1={major ? left : left - 4}
                    x2={major ? right : left}
                    y1={y}
                    y2={y}
                    stroke={
                      major
                        ? theme.palette.divider
                        : theme.palette.text.disabled
                    }
                  />
                  {major && (
                    <text
                      x={left - 10}
                      y={y + 4}
                      textAnchor="end"
                      fill={theme.palette.text.secondary}
                      fontSize="12"
                    >
                      {formatConnectionTraffic(scale.inverse(fraction))}
                    </text>
                  )}
                </g>
              )
            })}
            {areas.map((area) => (
              <g key={area.start} data-stacked-area="true">
                <path
                  data-area="upload"
                  d={area.upload}
                  fill={theme.palette.secondary.main}
                  fillOpacity="0.78"
                />
                <path
                  data-area="download"
                  d={area.download}
                  fill={theme.palette.primary.main}
                  fillOpacity="0.72"
                />
                <path
                  data-boundary="upload"
                  d={area.uploadCurve}
                  fill="none"
                  stroke={theme.palette.secondary.main}
                  strokeWidth="1.5"
                />
                <path
                  data-boundary="total"
                  d={area.totalCurve}
                  fill="none"
                  stroke={theme.palette.primary.main}
                  strokeWidth="2"
                />
              </g>
            ))}
            {hovered && (
              <g pointerEvents="none">
                <line
                  x1={hoverX}
                  x2={hoverX}
                  y1={top}
                  y2={bottom}
                  stroke={theme.palette.text.secondary}
                  strokeDasharray="3 3"
                />
                {hoveredBar && (
                  <>
                    <circle
                      cx={hoverX}
                      cy={hoveredBar.top}
                      r="3.5"
                      fill={theme.palette.primary.main}
                      stroke={theme.palette.background.paper}
                    />
                    <circle
                      cx={hoverX}
                      cy={hoveredBar.split}
                      r="3"
                      fill={theme.palette.secondary.main}
                      stroke={theme.palette.background.paper}
                    />
                  </>
                )}
              </g>
            )}
            {timeTicks.map((tick) => {
              const x = left + tick.fraction * (right - left)
              return (
                <g key={tick.minute}>
                  <line
                    data-time-tick="true"
                    x1={x}
                    x2={x}
                    y1={bottom}
                    y2={bottom + (tick.label ? 6 : 3)}
                    stroke={theme.palette.text.disabled}
                  />
                  {tick.label && (
                    <text
                      data-axis-tick="true"
                      x={x}
                      y={bottom + 19}
                      textAnchor={
                        tick.fraction === 0
                          ? 'start'
                          : tick.fraction === 1
                            ? 'end'
                            : 'middle'
                      }
                      fill={theme.palette.text.secondary}
                      fontSize="12"
                    >
                      {tick.label}
                    </text>
                  )}
                </g>
              )
            })}
          </svg>
        )}
        {hovered && history && (
          <Box
            role="status"
            sx={{
              position: 'absolute',
              top: 8,
              left: Math.max(4, Math.min(chartWidth - 238, hoverX + 12)),
              width: 226,
              p: 1,
              pointerEvents: 'none',
              bgcolor: 'background.paper',
              border: 1,
              borderColor: 'divider',
              borderRadius: 1,
              boxShadow: 2,
            }}
          >
            <Typography variant="caption" sx={{ fontWeight: 600 }}>
              {hovered.label} ·{' '}
              {includeDirect
                ? t('traffic.exportAll')
                : t('traffic.exportProxy')}
            </Typography>
            {hovered.upload == null || hovered.download == null ? (
              <Typography variant="body2">{noRecord}</Typography>
            ) : (
              <>
                {[
                  { label: uploadLabel, value: hovered.upload },
                  { label: downloadLabel, value: hovered.download },
                  {
                    label: totalLabel,
                    value: hovered.upload + hovered.download,
                  },
                ].map((item) => (
                  <Box
                    key={item.label}
                    sx={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 1,
                    }}
                  >
                    <Typography variant="caption">{item.label}</Typography>
                    <Typography
                      variant="caption"
                      sx={{ fontVariantNumeric: 'tabular-nums' }}
                    >
                      {formatConnectionTraffic(item.value)}
                    </Typography>
                  </Box>
                ))}
                {hovered.legacyHour && (
                  <Typography variant="caption" color="text.secondary">
                    {t('traffic.legacyHour')}
                  </Typography>
                )}
              </>
            )}
          </Box>
        )}
      </Box>
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 0.5,
          flexWrap: 'wrap',
          color: 'text.secondary',
        }}
      >
        <TrafficRecovery points={recovery} />
        <Typography variant="caption">
          {includeDirect
            ? t('traffic.chartAllScope')
            : t('traffic.chartProxyScope')}{' '}
          {scale.kind !== 'linear' && ` ${t('traffic.nonLinearCompact')}`}
          {buckets.some((bucket) => bucket.legacyHour) &&
            ` ${t('traffic.legacyCompact')}`}
        </Typography>
        <Tooltip
          title={[
            t('traffic.trendNote'),
            scale.kind !== 'linear' ? t('traffic.nonLinearNote') : '',
            typeof selectedWindow === 'string'
              ? t('traffic.recentWindowNote')
              : '',
            range <= 3 ? t('traffic.hourlyHistoryNote') : '',
          ]
            .filter(Boolean)
            .join(' ')}
        >
          <IconButton size="small" aria-label={t('traffic.chartExplanation')}>
            <InfoOutlinedIcon sx={{ fontSize: 16 }} />
          </IconButton>
        </Tooltip>
      </Box>
    </Box>
  )
})
