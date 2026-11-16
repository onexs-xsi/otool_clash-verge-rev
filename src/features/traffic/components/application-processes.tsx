import {
  Box,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Table,
  TableHead,
  TableBody,
  TableRow,
  TableCell,
  Typography,
} from '@mui/material'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import {
  getTrafficApplicationProcesses,
  type TrafficApplicationProcess,
} from '@/services/traffic/api'

export function ApplicationProcesses({
  app,
  path,
}: {
  app: string
  path: string
}) {
  const { t } = useTranslation()
  const [items, setItems] = useState<TrafficApplicationProcess[] | null>(null)
  const [failed, setFailed] = useState(false)
  const [open, setOpen] = useState(false)
  useEffect(() => {
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const refresh = async () => {
      try {
        const result = await getTrafficApplicationProcesses(app, path)
        if (active) {
          setItems(result)
          setFailed(false)
        }
      } catch {
        if (active) setFailed(true)
      } finally {
        if (active) timer = setTimeout(refresh, 5000)
      }
    }
    void refresh()
    return () => {
      active = false
      if (timer) clearTimeout(timer)
    }
  }, [app, path])
  if (app.startsWith('WSL:') || (!app && !path)) return null
  const summary = items
    ?.map((item) => `${item.name || '—'} · PID ${item.pid}`)
    .join(' / ')
  return (
    <>
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 1,
          minWidth: 0,
          minHeight: 36,
          mb: 1,
        }}
      >
        <Typography
          variant="caption"
          title={summary}
          noWrap
          sx={{ minWidth: 0, flex: 1, color: 'text.secondary' }}
        >
          {failed
            ? t('traffic.processInfoError')
            : items === null
              ? t('traffic.loading')
              : items.length
                ? summary
                : t('traffic.processNotRunning')}
        </Typography>
        <Button
          size="small"
          sx={{ flexShrink: 0 }}
          disabled={!items?.length}
          onClick={() => setOpen(true)}
        >
          {t('traffic.processInfo')}
        </Button>
      </Box>
      <Dialog
        open={open}
        onClose={() => setOpen(false)}
        maxWidth="md"
        fullWidth
      >
        <DialogTitle>{t('traffic.processInfo')}</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            {t('traffic.processLiveNote')}
          </Typography>
          <Box sx={{ overflowX: 'auto' }}>
            <Table size="small" sx={{ minWidth: 650, tableLayout: 'fixed' }}>
              <TableHead>
                <TableRow>
                  <TableCell>PID / PPID</TableCell>
                  <TableCell>{t('traffic.processName')}</TableCell>
                  <TableCell>{t('traffic.processPath')}</TableCell>
                  <TableCell>{t('traffic.processParent')}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {items?.map((item) => (
                  <TableRow key={item.pid}>
                    <TableCell>
                      {item.pid} / {item.parentPid}
                    </TableCell>
                    <TableCell sx={{ overflowWrap: 'anywhere' }}>
                      {item.name}
                      <Typography variant="caption" component="div">
                        {t(
                          item.matchSource === 'installation-family'
                            ? 'traffic.processSameInstallation'
                            : item.matchSource === 'path'
                              ? 'traffic.processExactPath'
                              : 'traffic.processNameCandidate',
                        )}
                      </Typography>
                    </TableCell>
                    <TableCell
                      sx={{ overflowWrap: 'anywhere', userSelect: 'text' }}
                    >
                      {item.path || '—'}
                    </TableCell>
                    <TableCell sx={{ overflowWrap: 'anywhere' }}>
                      {item.parentName || '—'}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setOpen(false)}>{t('traffic.close')}</Button>
        </DialogActions>
      </Dialog>
    </>
  )
}
