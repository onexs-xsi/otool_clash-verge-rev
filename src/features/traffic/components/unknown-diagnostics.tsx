import {
  Alert,
  Box,
  Button,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { showNotice } from '@/services/notice-service'
import {
  getTrafficApplicationDiagnostics,
  getWslTrafficStatus,
  type TrafficDiagnostic,
  type WslTrafficStatus,
} from '@/services/traffic/api'
import { copyTrafficDiagnostics } from '@/services/traffic/export'

interface Props {
  app: string
  path: string
  day: string
  chains: string[]
}

export function UnknownDiagnostics({ app, path, day, chains }: Props) {
  const { t } = useTranslation()
  const [loaded, setLoaded] = useState<{
    key: string
    records: TrafficDiagnostic[]
  } | null>(null)
  const [status, setStatus] = useState<WslTrafficStatus | null>(null)
  const [failedKey, setFailedKey] = useState('')
  const [attempt, setAttempt] = useState(0)
  const key = JSON.stringify([app, path, day, chains])
  const records = loaded?.key === key ? loaded.records : null

  useEffect(() => {
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const refresh = async () => {
      const [diagnostics, wsl] = await Promise.allSettled([
        getTrafficApplicationDiagnostics(app, path, day, chains),
        getWslTrafficStatus(),
      ])
      if (!active) return
      if (diagnostics.status === 'fulfilled') {
        setLoaded({ key, records: diagnostics.value })
        setFailedKey('')
      } else setFailedKey(key)
      setStatus(wsl.status === 'fulfilled' ? wsl.value : null)
      timer = setTimeout(refresh, 5000)
    }
    void refresh()
    return () => {
      active = false
      if (timer) clearTimeout(timer)
    }
  }, [app, path, day, chains, key, attempt])

  const wslLabel = !status
    ? t('traffic.wslDiagnosticsUnavailable')
    : status.phase === 'disabled'
      ? t('traffic.wslStatusDisabled')
      : status.phase === 'tun_off'
        ? t('traffic.wslTunOff')
        : status.phase === 'monitoring'
          ? t('traffic.wslStatusMonitoring')
          : t('traffic.wslStatusWaiting')
  const reason = (record: TrafficDiagnostic) =>
    record.inboundType === 'Inner'
      ? t('traffic.reasonInternal')
      : record.reason === 'wsl_process_unresolved'
        ? t('traffic.reasonWslProcess')
        : record.reason === 'missing_endpoints'
          ? t('traffic.reasonMissingEndpoints')
          : t('traffic.reasonProcess')

  return (
    <Box sx={{ mb: 1.5 }}>
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 1,
          mb: 0.5,
        }}
      >
        <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
          {t('traffic.diagnosticsTitle')}
        </Typography>
        <Box sx={{ display: 'flex', flexShrink: 0 }}>
          <Button size="small" onClick={() => setAttempt((value) => value + 1)}>
            {t('traffic.detectAgain')}
          </Button>
          <Button
            size="small"
            onClick={() =>
              void copyTrafficDiagnostics(
                JSON.stringify(
                  { app, path, day, records, wsl: status },
                  null,
                  2,
                ),
              )
                .then(() => showNotice.success(t('traffic.diagnosticsCopied')))
                .catch(showNotice.error)
            }
          >
            {t('traffic.copyDiagnostics')}
          </Button>
        </Box>
      </Box>
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ display: 'block', mb: 0.5 }}
      >
        {t('traffic.diagnosticsWsl', { status: wslLabel })}
        {status?.phase === 'monitoring' &&
          ` · ${status.distro ?? ''} · ${status.natMappingAvailable ? t('traffic.natAvailable') : t('traffic.natUnavailable')}`}
      </Typography>
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ display: 'block', mb: 1 }}
      >
        {t('traffic.diagnosticsNote')}
      </Typography>
      <Box sx={{ height: 196, mb: 1, overflow: 'auto' }}>
        {failedKey === key ? (
          <Alert severity="error">{t('traffic.diagnosticsError')}</Alert>
        ) : records === null ? (
          <Typography variant="body2" color="text.secondary">
            {t('traffic.loading')}
          </Typography>
        ) : records.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            {t('traffic.diagnosticsEmpty')}
          </Typography>
        ) : (
          <TableContainer
            sx={{
              bgcolor: 'background.paper',
              borderRadius: 1,
              height: 196,
              scrollbarGutter: 'stable',
            }}
          >
            <Table
              size="small"
              stickyHeader
              sx={{ minWidth: 850, tableLayout: 'fixed' }}
              aria-label={t('traffic.diagnosticsTitle')}
            >
              <TableHead>
                <TableRow>
                  <TableCell>{t('traffic.destination')}</TableCell>
                  <TableCell>{t('traffic.chain')}</TableCell>
                  <TableCell>{t('traffic.processInfo')}</TableCell>
                  <TableCell>{t('traffic.recognition')}</TableCell>
                  <TableCell>{t('traffic.lastSeen')}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {records.map((record) => (
                  <TableRow
                    key={JSON.stringify([
                      record.chain,
                      record.destination,
                      record.network,
                      record.reason,
                    ])}
                  >
                    <TableCell>
                      <Typography variant="body2">
                        {record.destination || '—'}
                      </Typography>
                      <Typography variant="caption" color="text.secondary">
                        {[
                          record.network.toUpperCase(),
                          record.source
                            ? t('traffic.sourceAddress', {
                                address: record.source,
                              })
                            : '',
                        ]
                          .filter(Boolean)
                          .join(' · ')}
                      </Typography>
                    </TableCell>
                    <TableCell>{record.chain || '—'}</TableCell>
                    <TableCell sx={{ overflowWrap: 'anywhere' }}>
                      {record.liveProcess || record.process ? (
                        <>
                          <Typography variant="body2">
                            {(record.liveProcess || record.process)?.name ||
                              '—'}{' '}
                            · PID {(record.liveProcess || record.process)?.pid}
                          </Typography>
                          <Typography variant="caption" color="text.secondary">
                            {t(
                              record.liveProcess
                                ? 'traffic.liveMatched'
                                : 'traffic.sampledProcess',
                            )}
                          </Typography>
                          <Typography
                            variant="caption"
                            component="div"
                            sx={{
                              overflowWrap: 'anywhere',
                              userSelect: 'text',
                            }}
                          >
                            {(record.liveProcess || record.process)?.path ||
                              t('traffic.pathUnavailable')}
                          </Typography>
                        </>
                      ) : (
                        <Typography variant="body2" color="text.secondary">
                          {t('traffic.processNotRecorded')}
                        </Typography>
                      )}
                      <Typography variant="caption" component="div">
                        {record.connectionActive == null
                          ? '—'
                          : t(
                              record.connectionActive
                                ? 'traffic.connectionLive'
                                : 'traffic.connectionEnded',
                            )}{' '}
                        · {record.inboundType || '—'}
                      </Typography>
                    </TableCell>
                    <TableCell>{reason(record)}</TableCell>
                    <TableCell sx={{ whiteSpace: 'nowrap' }}>
                      {record.lastSeen || '—'}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        )}
      </Box>
    </Box>
  )
}
