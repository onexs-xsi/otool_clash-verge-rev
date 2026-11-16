import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Typography,
} from '@mui/material'
import dayjs from 'dayjs'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { formatConnectionTraffic } from '@/components/connection/connection-row-view'
import { mutate } from '@/services/mutate'
import { showNotice } from '@/services/notice-service'
import {
  getTrafficStorageInfo,
  type TrafficStorageInfo,
  compactTrafficDatabase,
} from '@/services/traffic/api'
import { saveTrafficDatabase } from '@/services/traffic/export'

export function TrafficStorage({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation()
  const [info, setInfo] = useState<TrafficStorageInfo | null>(null)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')
  const [integrity, setIntegrity] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    void getTrafficStorageInfo()
      .then((value) => {
        if (active) setInfo(value)
      })
      .catch((failure: unknown) => {
        if (active) setError(String(failure))
      })
      .finally(() => {
        if (active) setBusy(false)
      })
    return () => {
      active = false
    }
  }, [])
  const run = async (action: 'refresh' | 'check' | 'compact' | 'backup') => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      if (action === 'backup') {
        if (
          await saveTrafficDatabase(
            `traffic-${dayjs().format('YYYY-MM-DD')}.sqlite3`,
          )
        )
          showNotice.success(t('traffic.backupSaved'))
      } else {
        const value =
          action === 'compact'
            ? (
                await mutate(compactTrafficDatabase, {
                  id: 'traffic-history',
                  errorNotice: false,
                })
              ).value
            : await getTrafficStorageInfo(action === 'check')
        setInfo(value)
        if (action === 'check') setIntegrity(value.integrity)
      }
    } catch (failure) {
      setError(String(failure))
    } finally {
      setBusy(false)
    }
  }
  const metrics = info
    ? ([
        [
          'storageDisk',
          formatConnectionTraffic(info.databaseBytes + info.walBytes),
        ],
        ['storageSamples', info.sampleRows.toLocaleString()],
        ['storageDays', info.recordedDays.toLocaleString()],
        ['storageReusable', formatConnectionTraffic(info.reusableBytes)],
      ] as const)
    : []
  return (
    <Dialog open onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
        {t('traffic.storageManage')}
        {busy && <CircularProgress size={18} />}
      </DialogTitle>
      <DialogContent sx={{ height: 440, minHeight: 0 }}>
        {error && (
          <Alert severity="error" sx={{ mb: 1, overflowWrap: 'anywhere' }}>
            {error}
          </Alert>
        )}
        {integrity && (
          <Alert
            severity={integrity === 'ok' ? 'success' : 'error'}
            sx={{ mb: 1 }}
          >
            {integrity === 'ok' ? t('traffic.storageHealthy') : integrity}
          </Alert>
        )}
        <Box
          sx={{
            display: 'grid',
            gridTemplateColumns: 'repeat(2,minmax(0,1fr))',
            gap: 1,
            mb: 1.5,
          }}
        >
          {metrics.map(([key, value]) => (
            <Box
              key={key}
              sx={{ bgcolor: 'action.hover', borderRadius: 1, p: 1.25 }}
            >
              <Typography variant="caption" color="text.secondary">
                {t(`traffic.${key}`)}
              </Typography>
              <Typography
                variant="subtitle1"
                sx={{ fontWeight: 600, fontVariantNumeric: 'tabular-nums' }}
              >
                {value}
              </Typography>
            </Box>
          ))}
        </Box>
        {info && (
          <>
            <Typography variant="body2">
              {t('traffic.storageRange', {
                first: info.firstDay ?? '—',
                last: info.lastDay ?? '—',
              })}
            </Typography>
            <Typography variant="body2" sx={{ mb: 1 }}>
              {t('traffic.storageSaved', {
                time: info.lastSavedAt
                  ? dayjs(info.lastSavedAt).format('YYYY-MM-DD HH:mm:ss')
                  : '—',
              })}
            </Typography>
            <Typography
              variant="caption"
              component="div"
              sx={{
                color: 'text.secondary',
                overflowWrap: 'anywhere',
                userSelect: 'text',
                mb: 1,
              }}
            >
              {info.path}
            </Typography>
          </>
        )}
        <Typography variant="body2" color="text.secondary">
          {t('traffic.storageNote')}
        </Typography>
        <Typography
          variant="caption"
          component="div"
          color="text.secondary"
          sx={{ mt: 1 }}
        >
          {t('traffic.storageCompactNote')}
        </Typography>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mt: 2 }}>
          <Button
            size="small"
            variant="outlined"
            disabled={busy || !info}
            onClick={() => void run('backup')}
          >
            {t('traffic.backupDatabase')}
          </Button>
          <Button
            size="small"
            disabled={busy || !info}
            onClick={() => void run('check')}
          >
            {t('traffic.storageCheck')}
          </Button>
          <Button
            size="small"
            disabled={busy || !info}
            onClick={() => void run('compact')}
          >
            {t('traffic.storageCompact')}
          </Button>
          <Button
            size="small"
            disabled={busy}
            onClick={() => void run('refresh')}
          >
            {t('traffic.detectAgain')}
          </Button>
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('traffic.close')}</Button>
      </DialogActions>
    </Dialog>
  )
}
