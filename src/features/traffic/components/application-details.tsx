import {
  Box,
  Button,
  Collapse,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { type TrafficStatisticsRow } from '@/services/traffic/api'

import { trafficTotals } from '../lib/traffic-metrics'
import { applicationKey, groupTrafficRows } from '../lib/traffic-order'

import { ApplicationIcon } from './application-icon'
import { ApplicationProcesses } from './application-processes'
import { TrafficChart } from './traffic-chart'
import { TrafficValue } from './traffic-value'
import { UnknownDiagnostics } from './unknown-diagnostics'

interface Props {
  app: string
  path: string
  month: string
  day: string
  rows: TrafficStatisticsRow[]
  filterActive: boolean
}

export function ApplicationDetails({
  app,
  path,
  month,
  day,
  rows,
  filterActive,
}: Props) {
  const { t } = useTranslation()
  const [pathsOpen, setPathsOpen] = useState(false)
  const identity = applicationKey({ app, path })
  const appRows = useMemo(
    () => rows.filter((row) => applicationKey(row) === identity),
    [rows, identity],
  )
  const paths = useMemo(
    () => [...new Map(appRows.map((row) => [row.path, row])).values()],
    [appRows],
  )
  const chainsKey = JSON.stringify(
    filterActive ? [...new Set(appRows.map((row) => row.chain))].sort() : [],
  )
  const chains = useMemo<string[]>(() => JSON.parse(chainsKey), [chainsKey])
  const unknown =
    (!app && !path) || (app.startsWith('WSL:') && app.endsWith('/?'))
  const { proxy, all } = useMemo(() => trafficTotals(appRows), [appRows])
  const displayName =
    app === '@clash-core'
      ? t('traffic.internalApp')
      : (app.startsWith('WSL:')
          ? app.replace('WSL:', 'WSL · ').replace('/', ' · ')
          : (app || path).split(/[\\/]/).filter(Boolean).pop()) ||
        t('traffic.unknownApp', {
          defaultValue: 'Unknown application',
        })

  const metrics = [
    {
      label: t('traffic.currentDownload', {
        defaultValue: 'Current download',
      }),
      value: proxy.downloadRate,
      all: all.downloadRate,
      rate: true,
    },
    {
      label: t('traffic.currentUpload', {
        defaultValue: 'Current upload',
      }),
      value: proxy.uploadRate,
      all: all.uploadRate,
      rate: true,
    },
    {
      label: t('traffic.dayTotal', {
        day,
        defaultValue: '{{day}} total',
      }),
      value: proxy.dayDownload + proxy.dayUpload,
      all: all.dayDownload + all.dayUpload,
    },
    {
      label: t('traffic.monthTotal', {
        month,
        defaultValue: '{{month}} total',
      }),
      value: proxy.monthDownload + proxy.monthUpload,
      all: all.monthDownload + all.monthUpload,
    },
  ]
  return (
    <Box
      sx={{
        p: { xs: 1.5, md: 2 },
        bgcolor: 'action.hover',
        borderBottom: 1,
        borderColor: 'divider',
      }}
    >
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: {
            xs: 'minmax(0, 1fr)',
            md: 'minmax(0, 1fr) minmax(0, 3fr)',
          },
          alignItems: 'center',
          gap: 1.5,
          mb: 1.5,
        }}
      >
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            gap: 1,
            minWidth: 0,
          }}
        >
          <ApplicationIcon
            path={path || (app.includes('\\') ? app : '')}
            app={app}
          />
          <Box sx={{ minWidth: 0 }}>
            <Typography
              variant="subtitle1"
              title={displayName}
              sx={{ fontWeight: 600, overflowWrap: 'anywhere' }}
            >
              {displayName}
            </Typography>
            <Typography
              variant="caption"
              title={path}
              sx={{
                color: 'text.secondary',
                display: 'block',
                overflowWrap: 'anywhere',
                userSelect: 'text',
              }}
            >
              {path ||
                t('traffic.pathUnavailable', {
                  defaultValue: 'Executable path unavailable',
                })}
            </Typography>
          </Box>
        </Box>
        <Box
          sx={{
            display: 'grid',
            gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
            '@container (min-width: 1040px)': {
              gridTemplateColumns: 'repeat(4, minmax(0, 1fr))',
            },
            gap: 1,
            minWidth: 0,
          }}
        >
          {metrics.map((metric) => (
            <Box
              key={metric.label}
              sx={{
                bgcolor: 'background.paper',
                borderRadius: 1,
                px: 1.25,
                py: 0.6,
                minWidth: 0,
                minHeight: 72,
              }}
            >
              <Typography
                variant="caption"
                sx={{ color: 'text.secondary' }}
                noWrap
              >
                {metric.label}
              </Typography>
              <Typography
                variant="subtitle1"
                sx={{
                  fontWeight: 600,
                  fontVariantNumeric: 'tabular-nums',
                  minHeight: 40,
                }}
              >
                <TrafficValue
                  value={metric.value}
                  all={metric.all}
                  rate={metric.rate}
                  wrap
                />
              </Typography>
            </Box>
          ))}
        </Box>
      </Box>

      {app === '@clash-core' ? (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
          {t('traffic.internalAppNote')}
        </Typography>
      ) : (
        <ApplicationProcesses app={app} path={path} />
      )}
      {paths.length > 1 && (
        <>
          <Button
            size="small"
            aria-expanded={pathsOpen}
            onClick={() => setPathsOpen((value) => !value)}
            sx={{ mb: 1 }}
          >
            {t('traffic.mergedPaths', { count: paths.length })}
          </Button>
          <Collapse in={pathsOpen} unmountOnExit>
            <Box
              sx={{
                mb: 1.5,
                bgcolor: 'background.paper',
                borderRadius: 1,
                p: 1.5,
              }}
            >
              <Typography
                variant="caption"
                color="text.secondary"
                component="div"
                sx={{ mb: 1 }}
              >
                {t('traffic.mergedPathsNote')}
              </Typography>
              {paths.map((item) => (
                <Box key={item.path} sx={{ mb: 1, '&:last-child': { mb: 0 } }}>
                  <Typography
                    variant="caption"
                    component="div"
                    sx={{ overflowWrap: 'anywhere', userSelect: 'text' }}
                  >
                    {item.path || t('traffic.pathUnavailable')}
                  </Typography>
                  <ApplicationProcesses app={item.app} path={item.path} />
                </Box>
              ))}
            </Box>
          </Collapse>
        </>
      )}
      {unknown && (
        <UnknownDiagnostics app={app} path={path} day={day} chains={chains} />
      )}

      <TrafficChart app={app} path={path} day={day} chains={chains} />

      <Typography variant="subtitle1" sx={{ mb: 0.5, fontWeight: 600 }}>
        {t('traffic.chainDetails', {
          defaultValue: 'Chain breakdown',
        })}
      </Typography>
      <TableContainer
        sx={{ bgcolor: 'background.paper', borderRadius: 1, minWidth: 0 }}
      >
        <Table
          size="small"
          sx={{
            minWidth: 780,
            tableLayout: 'fixed',
            '& th': { whiteSpace: 'nowrap' },
          }}
          aria-label={t('traffic.chainDetails', {
            defaultValue: 'Chain breakdown',
          })}
        >
          <colgroup>
            <col style={{ width: '36%' }} />
            {['dayDownload', 'dayUpload', 'monthDownload', 'monthUpload'].map(
              (column) => (
                <col key={column} style={{ width: '16%' }} />
              ),
            )}
          </colgroup>
          <TableHead>
            <TableRow>
              <TableCell>
                {t('traffic.chain', { defaultValue: 'Chain' })}
              </TableCell>
              <TableCell align="right">
                {t('traffic.dayDownload', {
                  defaultValue: 'Day download',
                })}
              </TableCell>
              <TableCell align="right">
                {t('traffic.dayUpload', {
                  defaultValue: 'Day upload',
                })}
              </TableCell>
              <TableCell align="right">
                {t('traffic.monthDownload', {
                  defaultValue: 'Month download',
                })}
              </TableCell>
              <TableCell align="right">
                {t('traffic.monthUpload', {
                  defaultValue: 'Month upload',
                })}
              </TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {groupTrafficRows(appRows, true).map((row) => (
              <TableRow key={row.chain}>
                <TableCell sx={{ overflowWrap: 'anywhere' }}>
                  {row.chain ||
                    t('traffic.unknownChain', {
                      defaultValue: 'Unknown chain',
                    })}
                </TableCell>
                <TableCell align="right">
                  <TrafficValue
                    tableCell
                    value={row.dayDownload}
                    all={row.all.dayDownload}
                  />
                </TableCell>
                <TableCell align="right">
                  <TrafficValue
                    tableCell
                    value={row.dayUpload}
                    all={row.all.dayUpload}
                  />
                </TableCell>
                <TableCell align="right">
                  <TrafficValue
                    tableCell
                    value={row.monthDownload}
                    all={row.all.monthDownload}
                  />
                </TableCell>
                <TableCell align="right">
                  <TrafficValue
                    tableCell
                    value={row.monthUpload}
                    all={row.all.monthUpload}
                  />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableContainer>
    </Box>
  )
}
