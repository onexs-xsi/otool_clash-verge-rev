import {
  Alert,
  Box,
  Button,
  LinearProgress,
  Stack,
  Typography,
} from '@mui/material'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog } from '@/components/base'
import { mutate } from '@/services/mutate'
import { errorDetail } from '@/services/notice-service'
import {
  cancelPortableUpdate,
  installPortableUpdate,
  preparePortableUpdate,
  type PortableUpdatePlan,
  type PortableUpdateProgress,
} from '@/services/traffic/portable-update'

export function PortableUpdateAction({ version }: { version: string }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [phase, setPhase] = useState<
    'preparing' | 'review' | 'installing' | 'cancelling' | 'failed'
  >('preparing')
  const [plan, setPlan] = useState<PortableUpdatePlan | null>(null)
  const [progress, setProgress] = useState<PortableUpdateProgress | null>(null)
  const [error, setError] = useState('')
  const generationRef = useRef(0)

  const prepare = async () => {
    const request = ++generationRef.current
    setOpen(true)
    setPlan(null)
    setProgress(null)
    setError('')
    setPhase('preparing')
    try {
      const result = await mutate(
        () =>
          preparePortableUpdate(version, (value) => {
            if (generationRef.current === request) setProgress(value)
          }),
        {
          id: 'portable-update',
          errorNotice: false,
        },
      )
      if (generationRef.current !== request) return
      if (result.ok && result.value) {
        setPlan(result.value)
        setPhase('review')
      } else {
        setOpen(false)
      }
    } catch (failure) {
      if (generationRef.current !== request) return
      setError(errorDetail(failure))
      setPhase('failed')
    }
  }

  const cancel = async () => {
    if (phase === 'installing' || phase === 'cancelling') return
    generationRef.current += 1
    setPhase('cancelling')
    try {
      await mutate(cancelPortableUpdate, {
        id: 'cancel-portable-update',
        errorNotice: false,
      })
      setOpen(false)
      setPlan(null)
    } catch (failure) {
      setError(errorDetail(failure))
      setPhase(plan ? 'review' : 'failed')
    }
  }

  const install = async () => {
    if (!plan || phase !== 'review') return
    setPhase('installing')
    setError('')
    try {
      await mutate(() => installPortableUpdate(plan.id), {
        id: 'portable-update',
        errorNotice: false,
      })
    } catch (failure) {
      setError(errorDetail(failure))
      setPhase('review')
    }
  }

  const downloading = progress?.stage === 'downloading' && progress.total > 0
  const percent = downloading
    ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
    : 0
  const progressLabel =
    phase === 'cancelling'
      ? t('traffic.updateCancelling')
      : progress?.stage === 'verifying'
        ? t('traffic.updateVerifying')
        : progress?.stage === 'extracting'
          ? t('traffic.updateExtracting')
          : t('traffic.updateDownloading')

  return (
    <>
      <Button
        variant="contained"
        disabled={open}
        onClick={() => void prepare()}
      >
        {t('traffic.replaceInstall')}
      </Button>
      <BaseDialog
        open={open}
        title={
          plan
            ? t('traffic.replaceConfirmTitle')
            : t('traffic.prepareUpdateTitle')
        }
        contentSx={{ width: 540, maxWidth: '100%' }}
        okBtn={
          phase === 'failed'
            ? t('traffic.retryUpdate')
            : t('traffic.confirmReplace')
        }
        cancelBtn={t('traffic.cancel')}
        cancelAutoFocus
        disableOk={phase === 'preparing' || phase === 'cancelling'}
        cancelDisabled={phase === 'installing' || phase === 'cancelling'}
        loading={phase === 'installing'}
        onOk={() => void (phase === 'failed' ? prepare() : install())}
        onCancel={() => void cancel()}
        onClose={() => void cancel()}
      >
        <Stack spacing={2}>
          {error && (
            <Alert severity="error" sx={{ overflowWrap: 'anywhere' }}>
              {t('traffic.replacementFailed')}
              <br />
              {error}
            </Alert>
          )}
          {plan ? (
            <>
              <Alert severity="warning">
                {t('traffic.replaceConfirmNote', {
                  version: `v${plan.version}`,
                })}
              </Alert>
              {(
                [
                  [t('traffic.replaceCurrentPath'), plan.currentExecutable],
                  [t('traffic.replaceSourcePath'), plan.sourceExecutable],
                  [t('traffic.replaceDownloadSource'), plan.downloadUrl],
                  [t('traffic.replaceBackupPath'), plan.backupDirectory],
                ] as const
              ).map(([label, value]) => (
                <Box key={label}>
                  <Typography variant="caption" color="text.secondary">
                    {label}
                  </Typography>
                  <Typography
                    variant="body2"
                    sx={{ overflowWrap: 'anywhere', userSelect: 'text' }}
                  >
                    {value}
                  </Typography>
                </Box>
              ))}
              <Typography variant="body2" color="text.secondary">
                {t('traffic.replacePermissionNote')}
              </Typography>
              {phase === 'installing' && (
                <Box role="status" aria-live="polite">
                  <Typography variant="body2" sx={{ mb: 1 }}>
                    {t('traffic.replacementStarting')}
                  </Typography>
                  <LinearProgress
                    aria-label={t('traffic.replacementStarting')}
                  />
                </Box>
              )}
            </>
          ) : (
            phase !== 'failed' && (
              <Box role="status" aria-live="polite">
                <Typography variant="body2" sx={{ mb: 1 }}>
                  {progressLabel}
                  {downloading ? ` ${percent}%` : ''}
                </Typography>
                <LinearProgress
                  aria-label={progressLabel}
                  variant={downloading ? 'determinate' : 'indeterminate'}
                  value={percent}
                />
                <Typography
                  variant="body2"
                  color="text.secondary"
                  sx={{ mt: 2 }}
                >
                  {t('traffic.prepareUpdateNote')}
                </Typography>
              </Box>
            )
          )}
        </Stack>
      </BaseDialog>
    </>
  )
}
