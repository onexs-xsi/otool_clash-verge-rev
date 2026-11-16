import CloseRoundedIcon from '@mui/icons-material/CloseRounded'
import ExpandLessRoundedIcon from '@mui/icons-material/ExpandLessRounded'
import ExpandMoreRoundedIcon from '@mui/icons-material/ExpandMoreRounded'
import FilterListRoundedIcon from '@mui/icons-material/FilterListRounded'
import PushPinRoundedIcon from '@mui/icons-material/PushPinRounded'
import ViewColumnRoundedIcon from '@mui/icons-material/ViewColumnRounded'
import {
  Alert,
  Box,
  Button,
  Checkbox,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControlLabel,
  IconButton,
  InputAdornment,
  ListItemText,
  Menu,
  MenuItem,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TablePagination,
  TableRow,
  TableSortLabel,
  TextField,
  Typography,
  Switch,
  Tooltip,
} from '@mui/material'
import dayjs from 'dayjs'
import { Fragment, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BasePage } from '@/components/base'
import { ApplicationDetails } from '@/features/traffic/components/application-details'
import { ApplicationIcon } from '@/features/traffic/components/application-icon'
import { TrafficStorage } from '@/features/traffic/components/traffic-storage'
import { TrafficValue } from '@/features/traffic/components/traffic-value'
import { WslMonitorControl } from '@/features/traffic/components/wsl-monitor-control'
import { trafficCsv } from '@/features/traffic/lib/traffic-csv'
import { normalizeApplicationKey } from '@/features/traffic/lib/traffic-identity'
import { trafficColumnWidths } from '@/features/traffic/lib/traffic-layout'
import {
  hasProxyTraffic,
  formatTrafficDuration,
  sumTrafficRows,
  trafficRoute,
} from '@/features/traffic/lib/traffic-metrics'
import {
  applicationKey,
  groupTrafficRows,
  orderTrafficRows,
  type TrafficDisplayRow,
  type TrafficSort,
  type TrafficSortField,
} from '@/features/traffic/lib/traffic-order'
import { recentActivity } from '@/features/traffic/lib/traffic-recent'
import { useRuntimeConfig } from '@/hooks/use-clash'
import { useVerge } from '@/hooks/use-verge'
import { patchClashConfig } from '@/services/cmds'
import { mutate } from '@/services/mutate'
import { showNotice } from '@/services/notice-service'
import { type TrafficStatisticsRow } from '@/services/traffic/api'
import { saveTrafficCsv } from '@/services/traffic/export'

import { TrafficStorageIndicator } from './components/traffic-storage-indicator'
import { useTrafficHistory } from './hooks/use-traffic-history'

const localDay = () => dayjs().format('YYYY-MM-DD')
const noPinnedApps: string[] = []
type NumericColumn = Exclude<TrafficSortField, 'app' | 'chain'>
const numericColumns: NumericColumn[] = [
  'dayDurationMs',
  'downloadRate',
  'uploadRate',
  'dayDownload',
  'dayUpload',
  'dayTotal',
  'monthDownload',
  'monthUpload',
]
const defaultColumns: TrafficSortField[] = [
  'chain',
  ...numericColumns.filter(
    (column) => column !== 'monthDownload' && column !== 'monthUpload',
  ),
]
const displayApp = (
  row: TrafficStatisticsRow,
  unknown: string,
  internal = 'Clash · Mihomo',
) => {
  if (row.app === '@clash-core') return internal
  const name = row.app || row.path
  if (name.startsWith('WSL:'))
    return name
      .replace('WSL:', 'WSL · ')
      .replace('/', ' · ')
      .replace(/\?$/, unknown)
  return name.split(/[\\/]/).filter(Boolean).pop() || unknown
}
const iconPath = (row: TrafficStatisticsRow) =>
  row.path || (row.app.includes('\\') ? row.app : '')

const TrafficPage = () => {
  const { t } = useTranslation()
  const { data: runtimeConfig } = useRuntimeConfig()
  const { verge, patchVerge } = useVerge()
  const [day, setDay] = useState(localDay)
  const [month, setMonth] = useState(() => localDay().slice(0, 7))
  const [enabling, setEnabling] = useState(false)
  const [enableFailed, setEnableFailed] = useState(false)
  const [searchInput, setSearchInput] = useState('')
  const [search, setSearch] = useState('')
  const [page, setPage] = useState(0)
  const [rowsPerPage, setRowsPerPage] = useState(25)
  const [expandedRow, setExpandedRow] = useState<string | null>(null)
  const [sort, setSort] = useState<TrafficSort | null>({
    field: 'dayTotal',
    direction: 'desc',
  })
  const [selectedChains, setSelectedChains] = useState<string[]>([])
  const [filterAnchor, setFilterAnchor] = useState<HTMLElement | null>(null)
  const [columnMenu, setColumnMenu] = useState<{ x: number; y: number } | null>(
    null,
  )
  const [columnBusy, setColumnBusy] = useState(false)
  const [menu, setMenu] = useState<{
    key: string
    x: number
    y: number
  } | null>(null)
  const [pinBusy, setPinBusy] = useState(false)
  const [groupBusy, setGroupBusy] = useState(false)
  const [proxyFilterBusy, setProxyFilterBusy] = useState(false)
  const [recentBusy, setRecentBusy] = useState(false)
  const [exportBusy, setExportBusy] = useState(false)
  const [storageOpen, setStorageOpen] = useState(false)
  const [clearTarget, setClearTarget] = useState<'day' | 'all' | null>(null)
  const [clearBusy, setClearBusy] = useState(false)
  const composingRef = useRef(false)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const { view, failed, clear } = useTrafficHistory({
    day,
    month,
    onDayChange: setDay,
    onMonthChange: setMonth,
    onResetPage: setPage,
  })

  const rows = useMemo(() => {
    const term = search.trim().toLocaleLowerCase()
    const applications = new Set(
      (view?.rows ?? [])
        .filter((row) =>
          [
            row.app,
            row.path,
            displayApp(row, t('traffic.unknownApp'), t('traffic.internalApp')),
          ].some((value) => value.toLocaleLowerCase().includes(term)),
        )
        .map(applicationKey),
    )
    return (view?.rows ?? []).filter(
      (row) =>
        (selectedChains.length === 0 || selectedChains.includes(row.chain)) &&
        (!term ||
          applications.has(applicationKey(row)) ||
          row.chain.toLocaleLowerCase().includes(term)),
    )
  }, [search, selectedChains, view, t])
  const chainOptions = useMemo(
    () =>
      [
        ...new Set([
          ...(view?.rows ?? []).map((row) => row.chain),
          ...selectedChains,
        ]),
      ].sort((a, b) => a.localeCompare(b)),
    [selectedChains, view],
  )
  const pinnedKeys = useMemo(
    () => [
      ...new Set(
        (verge?.traffic_pinned_apps ?? noPinnedApps).map(
          normalizeApplicationKey,
        ),
      ),
    ],
    [verge?.traffic_pinned_apps],
  )
  const visibleColumns = verge?.traffic_visible_columns ?? defaultColumns
  const grouped = verge?.traffic_group_by_application ?? false
  const proxyOnly = verge?.traffic_hide_direct_only ?? false
  const isToday = day === localDay()
  const recentOnly = (verge?.traffic_recent_active_only ?? false) && isToday
  const recent = useMemo(
    () => recentActivity(view?.rows ?? [], Date.now()),
    [view],
  )
  const tableColumns: TrafficSortField[] = [
    'app',
    ...(visibleColumns.includes('chain') ? ['chain' as const] : []),
    ...numericColumns.filter((column) => visibleColumns.includes(column)),
  ]
  const tableMinWidth = tableColumns.reduce(
    (width, column) => width + trafficColumnWidths[column],
    0,
  )
  const displayRows = useMemo<TrafficDisplayRow[]>(
    () =>
      groupTrafficRows(rows, !grouped).filter(
        (row) =>
          (!proxyOnly || hasProxyTraffic(row)) &&
          (!recentOnly ||
            pinnedKeys.includes(applicationKey(row)) ||
            recent.applications.has(applicationKey(row))),
      ),
    [grouped, rows, proxyOnly, recentOnly, pinnedKeys, recent],
  )
  const totals = useMemo(
    () => ({
      proxy: sumTrafficRows(displayRows),
      all: sumTrafficRows(displayRows.map((row) => row.all)),
    }),
    [displayRows],
  )
  const ordered = useMemo(
    () => orderTrafficRows(displayRows, pinnedKeys, sort),
    [displayRows, pinnedKeys, sort],
  )
  const safePage = Math.min(
    page,
    Math.max(0, Math.ceil(ordered.unpinned.length / rowsPerPage) - 1),
  )
  const shown = [
    ...ordered.pinned,
    ...ordered.unpinned.slice(
      safePage * rowsPerPage,
      (safePage + 1) * rowsPerPage,
    ),
  ]
  const needsProcessRecognition =
    runtimeConfig != null && runtimeConfig['find-process-mode'] !== 'always'

  const enableProcessRecognition = async () => {
    setEnabling(true)
    setEnableFailed(false)
    try {
      await mutate(() => patchClashConfig({ 'find-process-mode': 'always' }), {
        id: 'patch-clash-config',
        revalidate: [['getRuntimeConfig']],
        errorNotice: false,
      })
    } catch {
      setEnableFailed(true)
    } finally {
      setEnabling(false)
    }
  }

  const labels = {
    app: t('traffic.app', { defaultValue: 'Application' }),
    chain: t('traffic.chain', { defaultValue: 'Chain' }),
    dayDurationMs: t('traffic.duration'),
    uploadRate: t('traffic.uploadRate', {
      defaultValue: 'Upload speed',
    }),
    downloadRate: t('traffic.downloadRate', {
      defaultValue: 'Download speed',
    }),
    dayUpload: t('traffic.dayUpload', {
      defaultValue: 'Day upload',
    }),
    dayTotal: t('traffic.daySum'),
    dayDownload: t('traffic.dayDownload', {
      defaultValue: 'Day download',
    }),
    monthUpload: t('traffic.monthUpload', {
      defaultValue: 'Month upload',
    }),
    monthDownload: t('traffic.monthDownload', {
      defaultValue: 'Month download',
    }),
  }
  const chooseSort = (field: TrafficSortField) => {
    setSort((current) => ({
      field,
      direction:
        current?.field === field
          ? current.direction === 'asc'
            ? 'desc'
            : 'asc'
          : field === 'app' || field === 'chain'
            ? 'asc'
            : 'desc',
    }))
    setPage(0)
  }
  const exportRows = async () => {
    if (exportBusy) return
    const proxy = t('traffic.exportProxy')
    const all = t('traffic.exportAll')
    const headers = [
      labels.app,
      t('traffic.exportPath'),
      labels.chain,
      t('traffic.day'),
      t('traffic.month'),
      ...[labels.dayDownload, labels.dayUpload, labels.dayTotal].map(
        (label) => `${proxy} ${label} (B)`,
      ),
      ...[labels.dayDownload, labels.dayUpload, labels.dayTotal].map(
        (label) => `${all} ${label} (B)`,
      ),
      ...[proxy, all].flatMap((scope) =>
        [labels.monthDownload, labels.monthUpload].map(
          (label) => `${scope} ${label} (B)`,
        ),
      ),
      ...[proxy, all].flatMap((scope) =>
        [labels.downloadRate, labels.uploadRate].map(
          (label) => `${scope} ${label} (B/s)`,
        ),
      ),
      `${proxy} ${labels.dayDurationMs} (ms)`,
      t('traffic.exportDurationBound'),
    ]
    const content = trafficCsv(
      [...ordered.pinned, ...ordered.unpinned],
      day,
      month,
      headers,
    )
    setExportBusy(true)
    try {
      const saved = await saveTrafficCsv(content, `traffic-${day}.csv`)
      if (saved) showNotice.success(t('traffic.exportSuccess'))
    } catch (error) {
      showNotice.error(error)
    } finally {
      setExportBusy(false)
    }
  }
  const togglePin = async () => {
    if (!menu || pinBusy) return
    const key = menu.key
    const next = pinnedKeys.includes(key)
      ? pinnedKeys.filter((item) => item !== key)
      : [...pinnedKeys, key]
    setPinBusy(true)
    try {
      await patchVerge({ traffic_pinned_apps: next })
      setMenu(null)
      setPage(0)
    } catch (error) {
      showNotice.error(error)
    } finally {
      setPinBusy(false)
    }
  }
  const headerCell = (
    field: TrafficSortField,
    label: string,
    numeric = false,
  ) => (
    <TableCell
      key={field}
      align={numeric ? 'right' : 'left'}
      sortDirection={sort?.field === field ? sort.direction : false}
      onContextMenu={
        field === 'app'
          ? (event) => {
              event.preventDefault()
              setColumnMenu({ x: event.clientX, y: event.clientY })
            }
          : undefined
      }
    >
      <TableSortLabel
        active={sort?.field === field}
        direction={sort?.field === field ? sort.direction : 'asc'}
        onClick={() => chooseSort(field)}
      >
        {label}
      </TableSortLabel>
      {field === 'app' && (
        <IconButton
          size="small"
          aria-label={t('traffic.chooseColumns')}
          title={t('traffic.columnsHint')}
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect()
            setColumnMenu({ x: rect.left, y: rect.bottom })
          }}
        >
          <ViewColumnRoundedIcon fontSize="small" />
        </IconButton>
      )}
    </TableCell>
  )

  const clearHistory = async () => {
    if (!clearTarget || clearBusy) return
    setClearBusy(true)
    try {
      if (await clear(clearTarget === 'day' ? day : null)) setClearTarget(null)
    } catch (error) {
      showNotice.error(error)
    } finally {
      setClearBusy(false)
    }
  }

  const toggleColumn = async (column: TrafficSortField) => {
    if (columnBusy) return
    const next = visibleColumns.includes(column)
      ? visibleColumns.filter((item) => item !== column)
      : [...visibleColumns, column]
    setColumnBusy(true)
    try {
      await patchVerge({ traffic_visible_columns: next })
      if (!next.includes(column) && sort?.field === column)
        setSort({ field: 'app', direction: 'asc' })
    } catch (error) {
      showNotice.error(error)
    } finally {
      setColumnBusy(false)
    }
  }

  return (
    <BasePage
      title={t('layout.components.navigation.tabs.traffic')}
      header={
        <Typography
          variant="body2"
          sx={{ color: view?.connected ? 'success.main' : 'text.secondary' }}
          aria-live="polite"
        >
          {view?.connected
            ? t('traffic.collecting', {
                defaultValue: 'Collecting',
              })
            : t('traffic.disconnected', {
                defaultValue: 'Waiting for Clash',
              })}
        </Typography>
      }
      contentStyle={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        height: '100%',
        minHeight: 0,
        minWidth: 0,
      }}
    >
      {needsProcessRecognition && (
        <Alert
          severity="info"
          action={
            <Button
              color="inherit"
              size="small"
              onClick={() => void enableProcessRecognition()}
              disabled={enabling}
            >
              {enabling
                ? t('traffic.enablingProcess', {
                    defaultValue: 'Enabling…',
                  })
                : t('traffic.enableProcess', {
                    defaultValue: 'Enable application recognition',
                  })}
            </Button>
          }
        >
          {t('traffic.processHint', {
            defaultValue:
              'Enable application recognition for new connections. Previously unidentified traffic remains under Unknown application.',
          })}
        </Alert>
      )}
      {enableFailed && (
        <Alert severity="error">
          {t('traffic.enableFailed', {
            defaultValue: 'Could not enable application recognition.',
          })}
        </Alert>
      )}
      {view?.historyError && (
        <Alert severity="error">
          {t('traffic.historyError', {
            defaultValue:
              'Could not read or save traffic history. Existing history was preserved.',
          })}
        </Alert>
      )}
      {failed && (
        <Alert severity="error">
          {t('traffic.connectionError', {
            defaultValue: 'Unable to load traffic statistics.',
          })}
        </Alert>
      )}

      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: {
            xs: 'repeat(2, minmax(0, 1fr))',
            sm: '180px 160px minmax(0, 1fr)',
          },
          alignItems: 'center',
          gap: 1.5,
        }}
      >
        <TextField
          label={t('traffic.day', { defaultValue: 'Day' })}
          type="date"
          size="small"
          value={day}
          onChange={(event) => {
            const value = event.target.value
            if (/^\d{4}-\d{2}-\d{2}$/.test(value)) {
              setDay(value)
              setMonth(value.slice(0, 7))
              setPage(0)
            }
          }}
          slotProps={{ inputLabel: { shrink: true } }}
          sx={(theme) => ({ '& input': { colorScheme: theme.palette.mode } })}
        />
        <TextField
          label={t('traffic.month', { defaultValue: 'Month' })}
          type="month"
          size="small"
          value={month}
          onChange={(event) => {
            const value = event.target.value
            if (/^\d{4}-\d{2}$/.test(value)) {
              setMonth(value)
              if (!day.startsWith(value)) setDay(`${value}-01`)
              setPage(0)
            }
          }}
          slotProps={{ inputLabel: { shrink: true } }}
          sx={(theme) => ({ '& input': { colorScheme: theme.palette.mode } })}
        />
        <TextField
          label={t('traffic.search', {
            defaultValue: 'Search applications or chains',
          })}
          size="small"
          value={searchInput}
          inputRef={searchInputRef}
          onCompositionStart={() => {
            composingRef.current = true
          }}
          onCompositionEnd={(event) => {
            composingRef.current = false
            setSearch((event.target as HTMLInputElement).value)
            setPage(0)
          }}
          onChange={(event) => {
            const value = event.target.value
            setSearchInput(value)
            if (!composingRef.current) {
              setSearch(value)
              setPage(0)
            }
          }}
          slotProps={{
            input: {
              endAdornment: searchInput ? (
                <InputAdornment position="end">
                  <IconButton
                    size="small"
                    aria-label={t('traffic.clearSearch', {
                      defaultValue: 'Clear search',
                    })}
                    onClick={() => {
                      setSearchInput('')
                      setSearch('')
                      setPage(0)
                      searchInputRef.current?.focus()
                    }}
                  >
                    <CloseRoundedIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ) : null,
            },
          }}
          sx={{ minWidth: 0, gridColumn: { xs: '1 / -1', sm: 'auto' } }}
        />
      </Box>

      <Typography variant="caption" sx={{ color: 'text.secondary' }}>
        {t('traffic.proxyAccountingNote')}{' '}
        {t('traffic.samplingNote', {
          defaultValue:
            'Counts are estimates based on sampled connections. Recording continues while Clash Verge is running.',
        })}{' '}
        {t('traffic.expandHint', {
          defaultValue: 'Double-click an application to view details.',
        })}
      </Typography>
      {rows.some(
        (row) =>
          trafficRoute(row.chain) === 'unknown' &&
          row.dayDownload +
            row.dayUpload +
            row.monthDownload +
            row.monthUpload >
            0,
      ) && <Alert severity="info">{t('traffic.unknownOutboundNote')}</Alert>}

      <Box
        data-traffic-toolbar
        sx={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 1,
          flexShrink: 0,
        }}
      >
        <Box
          sx={{
            display: 'flex',
            flexWrap: 'wrap',
            alignItems: 'center',
            gap: 1,
            minWidth: 0,
          }}
        >
          <WslMonitorControl compact />
          <FormControlLabel
            sx={{ m: 0 }}
            control={
              <Switch
                size="small"
                checked={proxyOnly}
                disabled={proxyFilterBusy || !verge}
                onChange={async (_, checked) => {
                  setProxyFilterBusy(true)
                  try {
                    await patchVerge({ traffic_hide_direct_only: checked })
                    setPage(0)
                  } catch (error) {
                    showNotice.error(error)
                  } finally {
                    setProxyFilterBusy(false)
                  }
                }}
              />
            }
            label={
              <Typography variant="body2" noWrap>
                {t('traffic.proxyAppsOnly')}
              </Typography>
            }
          />
          <FormControlLabel
            sx={{ m: 0 }}
            control={
              <Switch
                size="small"
                checked={grouped}
                disabled={groupBusy}
                onChange={async (_, checked) => {
                  setGroupBusy(true)
                  try {
                    await patchVerge({ traffic_group_by_application: checked })
                    setPage(0)
                    setExpandedRow(null)
                  } catch (error) {
                    showNotice.error(error)
                  } finally {
                    setGroupBusy(false)
                  }
                }}
              />
            }
            label={
              <Typography variant="body2" noWrap>
                {t('traffic.groupApps')}
              </Typography>
            }
          />
          <Tooltip
            title={
              isToday
                ? t('traffic.recentHint', {
                    minutes: Math.ceil(recent.windowMs / 60_000),
                  })
                : t('traffic.recentHistorical')
            }
          >
            <span>
              <FormControlLabel
                sx={{ m: 0 }}
                label={
                  <Typography variant="body2" noWrap>
                    {t('traffic.recentActive')}
                  </Typography>
                }
                control={
                  <Switch
                    size="small"
                    checked={recentOnly}
                    disabled={recentBusy || !verge || !isToday}
                    onChange={async (_, checked) => {
                      setRecentBusy(true)
                      try {
                        await patchVerge({
                          traffic_recent_active_only: checked,
                        })
                        setPage(0)
                      } catch (error) {
                        showNotice.error(error)
                      } finally {
                        setRecentBusy(false)
                      }
                    }}
                  />
                }
              />
            </span>
          </Tooltip>
        </Box>
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            flexShrink: 0,
            '& button': { whiteSpace: 'nowrap' },
          }}
        >
          <Button
            size="small"
            sx={{ minWidth: 104 }}
            disabled={!view?.loaded}
            onClick={() => setStorageOpen(true)}
          >
            {t('traffic.storageManage')}
          </Button>
          <Button
            size="small"
            sx={{ minWidth: 104 }}
            disabled={!view?.loaded || displayRows.length === 0 || exportBusy}
            title={t('traffic.exportHint')}
            onClick={() => void exportRows()}
          >
            {exportBusy ? t('traffic.exporting') : t('traffic.exportCsv')}
          </Button>
          <Button
            size="small"
            disabled={!view?.loaded}
            onClick={() => setClearTarget('day')}
          >
            {t('traffic.clearDay')}
          </Button>
          <Button
            size="small"
            disabled={!view?.loaded}
            onClick={() => setClearTarget('all')}
          >
            {t('traffic.clearAll')}
          </Button>
        </Box>
      </Box>
      {storageOpen && <TrafficStorage onClose={() => setStorageOpen(false)} />}
      <TrafficStorageIndicator
        path={view?.historyPath ?? null}
        bytes={view?.historyBytes ?? null}
      />
      {view?.loaded && rows.length > 0 && (
        <Box
          sx={{
            display: 'flex',
            gap: 2,
            alignItems: 'baseline',
            px: 0.5,
            minHeight: 24,
            flexShrink: 0,
          }}
        >
          <Typography variant="body2" noWrap sx={{ flexShrink: 0 }}>
            {t('traffic.filteredProxyDay')}{' '}
            <TrafficValue
              value={totals.proxy.dayDownload + totals.proxy.dayUpload}
              all={totals.all.dayDownload + totals.all.dayUpload}
            />
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap>
            {t('traffic.summaryFollowsFilters')}
          </Typography>
        </Box>
      )}
      {!view?.loaded && !failed ? (
        <Box
          sx={{
            flex: 1,
            display: 'grid',
            placeItems: 'center',
            minHeight: 120,
          }}
        >
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <CircularProgress size={20} />
            <Typography>
              {t('traffic.loading', {
                defaultValue: 'Loading traffic history…',
              })}
            </Typography>
          </Box>
        </Box>
      ) : displayRows.length === 0 ? (
        <Box
          sx={{
            flex: 1,
            display: 'grid',
            placeItems: 'center',
            minHeight: 120,
          }}
        >
          <Box sx={{ textAlign: 'center' }}>
            <Typography sx={{ color: 'text.secondary' }}>
              {recentOnly
                ? t('traffic.recentEmpty')
                : proxyOnly
                  ? t('traffic.noProxyResults')
                  : search.trim() || selectedChains.length > 0
                    ? t('traffic.noResults', {
                        defaultValue:
                          'No applications or chains match the search.',
                      })
                    : t('traffic.empty', {
                        defaultValue:
                          'No traffic has been recorded yet. New connections will appear here.',
                      })}
            </Typography>
            {selectedChains.length > 0 && (
              <Button size="small" onClick={() => setSelectedChains([])}>
                {t('traffic.resetChains')}
              </Button>
            )}
          </Box>
        </Box>
      ) : (
        <>
          <TableContainer
            sx={{
              flex: 1,
              minHeight: 120,
              minWidth: 0,
              overflow: 'auto',
              scrollbarGutter: 'stable',
              borderRadius: 1,
              containerType: 'inline-size',
            }}
          >
            <Table
              stickyHeader
              size="small"
              aria-label={t('layout.components.navigation.tabs.traffic')}
              sx={{
                minWidth: tableMinWidth,
                tableLayout: 'fixed',
                '& th': { whiteSpace: 'nowrap' },
                '& td': { fontVariantNumeric: 'tabular-nums' },
              }}
            >
              <colgroup>
                {tableColumns.map((column) => (
                  <col
                    key={column}
                    style={{
                      width: `${(trafficColumnWidths[column] / tableMinWidth) * 100}%`,
                    }}
                  />
                ))}
              </colgroup>
              <TableHead>
                <TableRow>
                  {headerCell('app', labels.app)}
                  {visibleColumns.includes('chain') && (
                    <TableCell
                      sortDirection={
                        sort?.field === 'chain' ? sort.direction : false
                      }
                    >
                      <Box sx={{ display: 'flex', alignItems: 'center' }}>
                        <TableSortLabel
                          active={sort?.field === 'chain'}
                          direction={
                            sort?.field === 'chain' ? sort.direction : 'asc'
                          }
                          onClick={() => chooseSort('chain')}
                        >
                          {labels.chain}
                        </TableSortLabel>
                        <IconButton
                          size="small"
                          color={selectedChains.length ? 'primary' : 'default'}
                          aria-label={t('traffic.filterChains', {
                            defaultValue: 'Filter chains',
                          })}
                          aria-haspopup="menu"
                          aria-expanded={Boolean(filterAnchor)}
                          onClick={(event) =>
                            setFilterAnchor(event.currentTarget)
                          }
                        >
                          <FilterListRoundedIcon fontSize="small" />
                        </IconButton>
                        {selectedChains.length > 0 && (
                          <Typography
                            variant="caption"
                            sx={{ color: 'text.secondary' }}
                          >
                            {selectedChains.length}
                          </Typography>
                        )}
                      </Box>
                    </TableCell>
                  )}
                  {numericColumns
                    .filter((column) => visibleColumns.includes(column))
                    .map((column) => headerCell(column, labels[column], true))}
                </TableRow>
              </TableHead>
              <TableBody>
                {shown.map((row, index) => {
                  const rowKey = `${applicationKey(row)}\u0000${row.chain}`
                  const key = applicationKey(row)
                  const isPinned = pinnedKeys.includes(key)
                  const expanded = expandedRow === rowKey
                  const toggle = () =>
                    setExpandedRow((current) =>
                      current === rowKey ? null : rowKey,
                    )
                  return (
                    <Fragment key={rowKey}>
                      {ordered.pinned.length > 0 &&
                        index === ordered.pinned.length && (
                          <TableRow>
                            <TableCell
                              colSpan={tableColumns.length}
                              sx={{
                                py: 0.8,
                                bgcolor: 'background.default',
                                borderBottom: 0,
                              }}
                            >
                              <Typography
                                variant="caption"
                                sx={{ color: 'text.secondary' }}
                              >
                                {t('traffic.otherApps', {
                                  defaultValue: 'Other applications',
                                })}
                              </Typography>
                            </TableCell>
                          </TableRow>
                        )}
                      <TableRow
                        hover
                        onDoubleClick={toggle}
                        onContextMenu={(event) => {
                          event.preventDefault()
                          setMenu({ key, x: event.clientX, y: event.clientY })
                        }}
                        onKeyDown={(event) => {
                          if (
                            event.key !== 'ContextMenu' &&
                            !(event.shiftKey && event.key === 'F10')
                          )
                            return
                          event.preventDefault()
                          const rect =
                            event.currentTarget.getBoundingClientRect()
                          setMenu({ key, x: rect.left + 40, y: rect.bottom })
                        }}
                        sx={{ cursor: 'pointer', height: 53 }}
                      >
                        <TableCell>
                          <Box
                            sx={{
                              display: 'flex',
                              alignItems: 'center',
                              gap: 1,
                              minWidth: 0,
                            }}
                          >
                            <IconButton
                              size="small"
                              sx={{ flexShrink: 0 }}
                              aria-expanded={expanded}
                              aria-keyshortcuts="Shift+F10"
                              aria-label={
                                expanded
                                  ? t('traffic.collapseDetails', {
                                      defaultValue:
                                        'Collapse application details',
                                    })
                                  : t('traffic.expandDetails', {
                                      defaultValue:
                                        'Expand application details',
                                    })
                              }
                              onClick={(event) => {
                                event.stopPropagation()
                                toggle()
                              }}
                              onDoubleClick={(event) => event.stopPropagation()}
                            >
                              {expanded ? (
                                <ExpandLessRoundedIcon fontSize="small" />
                              ) : (
                                <ExpandMoreRoundedIcon fontSize="small" />
                              )}
                            </IconButton>
                            <ApplicationIcon
                              path={iconPath(row)}
                              app={row.app}
                            />
                            <Typography
                              variant="body2"
                              title={row.path || row.app}
                              noWrap
                              sx={{ minWidth: 0, flex: 1 }}
                            >
                              {displayApp(
                                row,
                                t('traffic.unknownApp', {
                                  defaultValue: 'Unknown application',
                                }),
                                t('traffic.internalApp'),
                              )}
                            </Typography>
                            {isPinned && (
                              <PushPinRoundedIcon
                                fontSize="small"
                                color="action"
                                sx={{ flexShrink: 0 }}
                              />
                            )}
                          </Box>
                        </TableCell>
                        {visibleColumns.includes('chain') && (
                          <TableCell>
                            <Typography
                              variant="body2"
                              title={row.chain}
                              noWrap
                            >
                              {row.chains && row.chains.length > 1
                                ? t('traffic.chainCount', {
                                    count: row.chains.length,
                                  })
                                : row.chain ||
                                  t('traffic.unknownChain', {
                                    defaultValue: 'Unknown chain',
                                  })}
                            </Typography>
                          </TableCell>
                        )}
                        {numericColumns
                          .filter((column) => visibleColumns.includes(column))
                          .map((column) => (
                            <TableCell
                              key={column}
                              align="right"
                              title={
                                column === 'dayDurationMs'
                                  ? row.chains && row.chains.length > 1
                                    ? t('traffic.groupDurationHint')
                                    : row.durationIncomplete
                                      ? t('traffic.durationIncomplete')
                                      : t('traffic.durationHint')
                                  : undefined
                              }
                            >
                              {column === 'dayDurationMs' ? (
                                formatTrafficDuration(
                                  row.dayDurationMs,
                                  row.durationIncomplete,
                                )
                              ) : column === 'dayTotal' ? (
                                <TrafficValue
                                  tableCell
                                  value={row.dayDownload + row.dayUpload}
                                  all={row.all.dayDownload + row.all.dayUpload}
                                />
                              ) : (
                                <TrafficValue
                                  tableCell
                                  value={row[column]}
                                  all={row.all[column]}
                                  rate={
                                    column === 'downloadRate' ||
                                    column === 'uploadRate'
                                  }
                                />
                              )}
                            </TableCell>
                          ))}
                      </TableRow>
                      {expanded && (
                        <TableRow>
                          <TableCell
                            colSpan={tableColumns.length}
                            sx={{ p: 0, whiteSpace: 'normal' }}
                          >
                            <Box
                              sx={{
                                position: 'sticky',
                                left: 0,
                                width: '100cqw',
                                maxWidth: '100%',
                              }}
                            >
                              <ApplicationDetails
                                key={`${applicationKey(row)}\u0000${month}`}
                                app={row.app}
                                path={row.path}
                                month={month}
                                day={day}
                                rows={rows}
                                filterActive={
                                  selectedChains.length > 0 ||
                                  Boolean(search.trim())
                                }
                              />
                            </Box>
                          </TableCell>
                        </TableRow>
                      )}
                    </Fragment>
                  )
                })}
              </TableBody>
            </Table>
          </TableContainer>
          {ordered.unpinned.length > 0 && (
            <TablePagination
              component="div"
              count={ordered.unpinned.length}
              page={safePage}
              onPageChange={(_, next) => setPage(next)}
              rowsPerPage={rowsPerPage}
              onRowsPerPageChange={(event) => {
                setRowsPerPage(Number(event.target.value))
                setPage(0)
              }}
              rowsPerPageOptions={[25, 50, 100]}
              labelRowsPerPage={t('traffic.unpinnedRowsPerPage', {
                defaultValue: 'Unpinned rows per page',
              })}
            />
          )}
        </>
      )}
      <Menu
        open={menu !== null}
        onClose={() => !pinBusy && setMenu(null)}
        anchorReference="anchorPosition"
        anchorPosition={menu ? { top: menu.y, left: menu.x } : undefined}
      >
        <MenuItem disabled={pinBusy} onClick={() => void togglePin()}>
          {menu && pinnedKeys.includes(menu.key)
            ? t('traffic.unpin', {
                defaultValue: 'Unpin application',
              })
            : t('traffic.pin', {
                defaultValue: 'Pin application',
              })}
        </MenuItem>
      </Menu>
      <Menu
        anchorEl={filterAnchor}
        open={Boolean(filterAnchor)}
        onClose={() => setFilterAnchor(null)}
        slotProps={{ paper: { sx: { maxHeight: 400, minWidth: 220 } } }}
      >
        <MenuItem
          disabled={selectedChains.length === 0}
          onClick={() => {
            setSelectedChains([])
            setPage(0)
          }}
        >
          {t('traffic.clearChainFilter', {
            defaultValue: 'Show all chains',
          })}
        </MenuItem>
        {chainOptions.map((chain) => (
          <MenuItem
            key={chain || '__unknown_chain__'}
            role="menuitemcheckbox"
            aria-checked={selectedChains.includes(chain)}
            onClick={() => {
              setSelectedChains((current) =>
                current.includes(chain)
                  ? current.filter((item) => item !== chain)
                  : [...current, chain],
              )
              setPage(0)
            }}
          >
            <Checkbox
              size="small"
              checked={selectedChains.includes(chain)}
              tabIndex={-1}
            />
            <ListItemText
              primary={
                chain ||
                t('traffic.unknownChain', {
                  defaultValue: 'Unknown chain',
                })
              }
            />
          </MenuItem>
        ))}
      </Menu>
      <Menu
        open={columnMenu !== null}
        onClose={() => !columnBusy && setColumnMenu(null)}
        anchorReference="anchorPosition"
        anchorPosition={
          columnMenu ? { top: columnMenu.y, left: columnMenu.x } : undefined
        }
        slotProps={{ paper: { sx: { maxHeight: 440, minWidth: 220 } } }}
      >
        {(['chain', ...numericColumns] as TrafficSortField[]).map((column) => (
          <MenuItem
            key={column}
            role="menuitemcheckbox"
            aria-checked={visibleColumns.includes(column)}
            disabled={columnBusy}
            onClick={() => void toggleColumn(column)}
          >
            <Checkbox
              size="small"
              checked={visibleColumns.includes(column)}
              tabIndex={-1}
            />
            <ListItemText primary={labels[column]} />
          </MenuItem>
        ))}
      </Menu>
      <Dialog
        open={clearTarget !== null}
        onClose={() => !clearBusy && setClearTarget(null)}
      >
        <DialogTitle>{t('traffic.clearConfirmTitle')}</DialogTitle>
        <DialogContent>
          <DialogContentText>
            {clearTarget === 'day'
              ? t('traffic.clearDayConfirm', { day })
              : t('traffic.clearAllConfirm')}
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button disabled={clearBusy} onClick={() => setClearTarget(null)}>
            {t('traffic.cancel')}
          </Button>
          <Button
            color="error"
            disabled={clearBusy}
            onClick={() => void clearHistory()}
          >
            {t('traffic.clear')}
          </Button>
        </DialogActions>
      </Dialog>
    </BasePage>
  )
}

export default TrafficPage
