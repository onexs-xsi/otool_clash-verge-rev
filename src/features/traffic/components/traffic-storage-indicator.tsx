import { Box, Tooltip, Typography } from '@mui/material'
import { useTranslation } from 'react-i18next'

import parseTraffic from '@/utils/parse-traffic'

export function TrafficStorageIndicator({
  path,
  bytes,
}: {
  path: string | null
  bytes: number | null
}) {
  const { t } = useTranslation()
  const size = bytes === null ? '—' : parseTraffic(bytes).join(' ')
  return (
    <Box
      data-traffic-storage
      sx={{
        display: 'flex',
        alignItems: 'baseline',
        gap: 0.5,
        minWidth: 0,
        minHeight: '1.5em',
        flexShrink: 0,
      }}
    >
      <Typography
        variant="caption"
        color="text.secondary"
        title={path ?? undefined}
        noWrap
        sx={{ minWidth: 0 }}
      >
        {path ? t('traffic.storage', { path }) : '\u00a0'}
      </Typography>
      {path && (
        <Tooltip title={t('traffic.storageFileSizeHint')}>
          <Typography
            component="span"
            variant="caption"
            color="text.secondary"
            sx={{
              flexShrink: 0,
              minWidth: 72,
              fontVariantNumeric: 'tabular-nums',
              whiteSpace: 'nowrap',
            }}
          >
            ({size})
          </Typography>
        </Tooltip>
      )}
    </Box>
  )
}
