import AppsOutlinedIcon from '@mui/icons-material/AppsOutlined'
import HubRoundedIcon from '@mui/icons-material/HubRounded'
import TerminalRoundedIcon from '@mui/icons-material/TerminalRounded'
import { Box, Tooltip, Typography } from '@mui/material'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { getTrafficAppIcon } from '@/services/traffic/api'
import { createTrafficIconLoader } from '@/services/traffic/icons'

const icons = createTrafficIconLoader(getTrafficAppIcon)

export const ApplicationIcon = ({
  path,
  app = '',
}: {
  path: string
  app?: string
}) => {
  const { t } = useTranslation()
  const internal = app === '@clash-core'
  const name =
    (app || path)
      .split(/[\\/]/)
      .filter(Boolean)
      .at(-1)
      ?.replace(/\.exe$/i, '') ?? ''
  const key = JSON.stringify([path, app])
  const [loaded, setLoaded] = useState<{
    key: string
    icon: string | null
  } | null>(null)
  const icon =
    loaded?.key === key ? loaded.icon : (icons.peek(path, app) ?? null)

  useEffect(() => {
    if (
      internal ||
      (!path && !app.toLowerCase().endsWith('.exe')) ||
      path.startsWith('wsl://') ||
      app.startsWith('WSL:')
    )
      return
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const refresh = async () => {
      const result = await icons.get(path, app)
      if (!active) return
      setLoaded({ key, icon: result })
      if (result === null) timer = setTimeout(() => void refresh(), 60_000)
    }
    void refresh()
    return () => {
      active = false
      if (timer) clearTimeout(timer)
    }
  }, [path, app, key, internal])

  return (
    <Tooltip
      title={
        internal
          ? t('traffic.internalApp')
          : !icon && name
            ? t('traffic.iconFallbackHint', { name })
            : ''
      }
    >
      <Box
        aria-hidden="true"
        sx={{
          width: 30,
          height: 30,
          flexShrink: 0,
          display: 'grid',
          placeItems: 'center',
          borderRadius: 1,
          bgcolor: 'action.hover',
          color: 'text.secondary',
        }}
      >
        {internal ? (
          <HubRoundedIcon fontSize="small" />
        ) : path.startsWith('wsl://') ? (
          <TerminalRoundedIcon fontSize="small" />
        ) : icon ? (
          <Box
            component="img"
            src={icon}
            alt=""
            sx={{ width: 24, height: 24 }}
          />
        ) : name && app !== '' ? (
          <Typography
            component="span"
            variant="caption"
            sx={{
              fontWeight: 700,
              color: 'primary.main',
              letterSpacing: '0.02em',
            }}
          >
            {Array.from(name).slice(0, 2).join('').toUpperCase()}
          </Typography>
        ) : (
          <AppsOutlinedIcon fontSize="small" />
        )}
      </Box>
    </Tooltip>
  )
}
