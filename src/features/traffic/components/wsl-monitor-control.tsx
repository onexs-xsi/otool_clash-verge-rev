import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControl,
  FormControlLabel,
  InputLabel,
  MenuItem,
  Select as MuiSelect,
  Stack,
  Switch,
  Typography,
} from '@mui/material'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useVerge } from '@/hooks/use-verge'
import { getRuntimeState, getVergeConfig } from '@/services/cmds'
import { showNotice } from '@/services/notice-service'
import { requestService } from '@/services/service-request'
import {
  getWslTrafficStatus,
  type WslTrafficStatus,
} from '@/services/traffic/api'
import getSystem from '@/utils/get-system'

const isWindows = getSystem() === 'windows'

export function WslMonitorControl({ compact = false }: { compact?: boolean }) {
  const { t } = useTranslation()
  const { verge, patchVerge } = useVerge()
  const [status, setStatus] = useState<WslTrafficStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [confirmTun, setConfirmTun] = useState(false)
  const enabled = verge?.enable_wsl_traffic_monitor ?? false
  const tunEnabled = verge?.enable_tun_mode ?? false

  useEffect(() => {
    if (!isWindows) return
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const refresh = async () => {
      try {
        const result = await getWslTrafficStatus()
        if (active) setStatus(result)
      } catch {
        if (active) setStatus(null)
      } finally {
        if (active) timer = setTimeout(refresh, 2000)
      }
    }
    void refresh()
    return () => {
      active = false
      if (timer) clearTimeout(timer)
    }
  }, [])

  if (!isWindows) return null

  const setMonitoring = async (checked: boolean) => {
    setBusy(true)
    try {
      if (checked) {
        const current = await getVergeConfig()
        const runtime = await getRuntimeState()
        if (!current.enable_tun_mode || !runtime.tunCapable) {
          setConfirmTun(true)
          return
        }
      }
      await patchVerge({ enable_wsl_traffic_monitor: checked })
    } catch (error) {
      showNotice.error(error)
    } finally {
      setBusy(false)
    }
  }

  const enableTunAndMonitor = async () => {
    setBusy(true)
    try {
      const current = await getVergeConfig()
      const runtime = await getRuntimeState()
      if (!runtime.tunCapable) {
        setConfirmTun(false)
        requestService({
          reason: 'tunNeedsService',
          restore: { enable_tun_mode: true, enable_wsl_traffic_monitor: true },
        })
        return
      }
      if (!current.enable_tun_mode) {
        await patchVerge({ enable_tun_mode: true })
        const updated = await getVergeConfig()
        if (!updated.enable_tun_mode) {
          showNotice.error(t('traffic.wslTunFailed'))
          return
        }
      }
      await patchVerge({ enable_wsl_traffic_monitor: true })
      setConfirmTun(false)
    } catch (error) {
      showNotice.error(error)
    } finally {
      setBusy(false)
    }
  }

  const label = !enabled
    ? t('traffic.wslStatusDisabled')
    : !tunEnabled || status?.phase === 'tun_off'
      ? t('traffic.wslTunOff')
      : status?.phase === 'monitoring'
        ? t('traffic.wslStatusMonitoring')
        : t('traffic.wslStatusWaiting')
  const toggle = (
    <FormControlLabel
      sx={{ m: 0 }}
      control={
        <Switch
          size="small"
          checked={enabled && tunEnabled}
          disabled={busy || !verge}
          onChange={(_, checked) => void setMonitoring(checked)}
        />
      }
      label={t('traffic.wslToggle')}
    />
  )
  const dialog = (
    <Dialog open={confirmTun} onClose={() => !busy && setConfirmTun(false)}>
      <DialogTitle>{t('traffic.wslTunTitle')}</DialogTitle>
      <DialogContent>
        <DialogContentText>{t('traffic.wslTunQuestion')}</DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button disabled={busy} onClick={() => setConfirmTun(false)}>
          {t('traffic.cancel')}
        </Button>
        <Button disabled={busy} onClick={() => void enableTunAndMonitor()}>
          {t('traffic.wslTunEnable')}
        </Button>
      </DialogActions>
    </Dialog>
  )
  if (compact) {
    return (
      <>
        {dialog}
        <Box
          sx={{ display: 'flex', alignItems: 'center', gap: 1, minWidth: 0 }}
        >
          {toggle}
          <Typography
            variant="caption"
            title={label}
            noWrap
            sx={{ width: '12em', flexShrink: 0 }}
            color={
              enabled && tunEnabled && status?.phase === 'monitoring'
                ? 'success.main'
                : 'text.secondary'
            }
          >
            {label}
          </Typography>
        </Box>
      </>
    )
  }
  const selected = verge?.wsl_monitor_distribution ?? ''
  const distributions = [
    ...new Set([
      ...(status?.distributions ?? []),
      ...(selected ? [selected] : []),
    ]),
  ]
  return (
    <Box sx={{ p: 2.5 }}>
      {dialog}
      <Typography variant="h6" sx={{ fontWeight: 600, mb: 1.5 }}>
        {t('traffic.wslTitle')}
      </Typography>
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 1,
        }}
      >
        <Typography
          variant="body2"
          color={
            enabled && tunEnabled && status?.phase === 'monitoring'
              ? 'success.main'
              : 'text.secondary'
          }
        >
          {label}
        </Typography>
        {toggle}
      </Box>
      <Typography variant="body2" color="text.secondary" sx={{ mt: 1.5 }}>
        {t('traffic.wslDescription')}
      </Typography>
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ display: 'block', mt: 1 }}
      >
        {t('traffic.wslRoot')}
      </Typography>
      {enabled && tunEnabled && (
        <Stack spacing={1.5} sx={{ mt: 2 }}>
          <FormControl size="small" fullWidth>
            <InputLabel id="wsl-distro-label" shrink>
              {t('traffic.wslDistro')}
            </InputLabel>
            <MuiSelect
              labelId="wsl-distro-label"
              displayEmpty
              label={t('traffic.wslDistro')}
              value={selected}
              disabled={busy}
              onChange={(event) =>
                void patchVerge({
                  wsl_monitor_distribution: event.target.value,
                }).catch(showNotice.error)
              }
            >
              <MenuItem value="">{t('traffic.wslAutomatic')}</MenuItem>
              {distributions.map((name) => (
                <MenuItem key={name} value={name}>
                  {name}
                </MenuItem>
              ))}
            </MuiSelect>
          </FormControl>
          {status?.phase === 'monitoring' && (
            <Typography variant="caption" color="text.secondary">
              {status.distro}
            </Typography>
          )}
          {status?.error && (
            <Alert severity="warning">
              {t('traffic.wslErrorHint')}
              <br />
              {status.error}
            </Alert>
          )}
          {status?.phase === 'monitoring' &&
            !status.directDetectionAvailable && (
              <Alert severity="warning">{t('traffic.wslIpMissing')}</Alert>
            )}
          {status?.phase === 'monitoring' && !status.natMappingAvailable && (
            <Alert severity="warning">{t('traffic.wslNatUnavailable')}</Alert>
          )}
          {status?.phase === 'waiting' && !status.error && (
            <Alert severity="info">{t('traffic.wslWaitingStart')}</Alert>
          )}
        </Stack>
      )}
    </Box>
  )
}
