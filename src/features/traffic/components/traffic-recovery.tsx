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
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { formatConnectionTraffic } from '@/components/connection/connection-row-view'
import type { TrafficSeriesPoint } from '@/services/traffic/api'

export function TrafficRecovery({ points }: { points: TrafficSeriesPoint[] }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const total = points.reduce(
    (sum, point) => sum + point.upload + point.download,
    0,
  )
  if (!total) return null
  return (
    <>
      <Button
        size="small"
        onClick={() => setOpen(true)}
        sx={{ textTransform: 'none', fontSize: '0.75rem' }}
      >
        {t('traffic.recoverySummary', {
          value: formatConnectionTraffic(total),
        })}
      </Button>
      <Dialog
        open={open}
        onClose={() => setOpen(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>{t('traffic.recoveryTitle')}</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            {t('traffic.recoveryNote')}
          </Typography>
          <Box sx={{ overflowX: 'auto' }}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{t('traffic.recoveryHour')}</TableCell>
                  <TableCell align="right">{t('traffic.upload')}</TableCell>
                  <TableCell align="right">{t('traffic.download')}</TableCell>
                  <TableCell align="right">{t('traffic.chartTotal')}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {points.map((point) => (
                  <TableRow key={point.period}>
                    <TableCell sx={{ whiteSpace: 'nowrap' }}>
                      {point.period.replace('T', ' ')}:00
                    </TableCell>
                    <TableCell align="right">
                      {formatConnectionTraffic(point.upload)}
                    </TableCell>
                    <TableCell align="right">
                      {formatConnectionTraffic(point.download)}
                    </TableCell>
                    <TableCell align="right">
                      {formatConnectionTraffic(point.upload + point.download)}
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
