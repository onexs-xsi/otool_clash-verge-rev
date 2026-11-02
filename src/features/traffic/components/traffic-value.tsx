import { Box } from '@mui/material'
import { useTranslation } from 'react-i18next'

import { formatConnectionTraffic } from '@/components/connection/connection-row-view'

export function TrafficValue({
  value,
  all,
  rate = false,
  wrap = false,
  tableCell = false,
}: {
  value: number
  all?: number
  rate?: boolean
  wrap?: boolean
  tableCell?: boolean
}) {
  const { t } = useTranslation()
  const format = (bytes: number) =>
    `${formatConnectionTraffic(bytes)}${rate ? '/s' : ''}`
  const hasOther = all != null && all > value + 0.0001
  return (
    <Box
      component="span"
      sx={{
        display: tableCell ? 'flex' : 'inline',
        flexWrap: tableCell ? 'wrap' : undefined,
        alignContent: 'center',
        alignItems: 'center',
        justifyContent: 'flex-end',
        columnGap: tableCell ? 0.6 : undefined,
        height: tableCell ? 40 : undefined,
        minWidth: 0,
        lineHeight: tableCell ? '20px' : undefined,
        whiteSpace: wrap || tableCell ? 'normal' : 'nowrap',
        fontVariantNumeric: 'tabular-nums',
      }}
      title={
        hasOther
          ? t('traffic.proxyValueHint', {
              proxy: format(value),
              all: format(all),
            })
          : undefined
      }
    >
      <Box component="span" sx={{ whiteSpace: 'nowrap' }}>
        {format(value)}
      </Box>
      {hasOther && (
        <Box
          component="span"
          sx={{
            color: 'text.secondary',
            ml: tableCell ? 0 : 0.6,
            whiteSpace: 'nowrap',
          }}
        >
          ({format(all)})
        </Box>
      )}
    </Box>
  )
}
