import {
  Alert,
  Box,
  Button,
  LinearProgress,
  Link,
  Stack,
  Typography,
} from '@mui/material'
import { lazy, type Ref, Suspense, useImperativeHandle, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog, type DialogRef } from '@/components/base'
import { useUpdate } from '@/hooks/use-update'
import { showNotice } from '@/services/notice-service'
import { openExternalUrl } from '@/utils/open-external-url'
import { version } from '@root/package.json'

import {
  trafficDisplayName,
  trafficReleasesUrl,
  trafficUpstreamVersion,
} from '../edition'

const LazyReactMarkdown = lazy(() => import('react-markdown'))

export function TrafficUpdateViewer({ ref }: { ref?: Ref<DialogRef> }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const releasesUrl = trafficReleasesUrl
  const { updateInfo, loading, error, refetch } = useUpdate({
    enabled: open,
    automatic: false,
  })
  const release = updateInfo && 'downloadUrl' in updateInfo ? updateInfo : null
  const openUrl = (url: string) => {
    void openExternalUrl(url).catch(showNotice.error)
  }
  useImperativeHandle(ref, () => ({
    open: () => setOpen(true),
    close: () => setOpen(false),
  }))
  return (
    <BaseDialog
      open={open}
      title={t('traffic.manualUpdates')}
      contentSx={{ width: 520, maxWidth: 'calc(100vw - 56px)' }}
      disableCancel
      okBtn={t('traffic.close')}
      onClose={() => setOpen(false)}
      onOk={() => setOpen(false)}
    >
      <Typography variant="subtitle1" sx={{ mb: 1 }}>
        {trafficDisplayName}
      </Typography>
      <Typography variant="body2">
        {t('traffic.currentVersion', { version: `v${version}` })}
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        {t('traffic.upstreamVersion', { version: trafficUpstreamVersion })}
      </Typography>
      <Box aria-live="polite" sx={{ mb: 2 }}>
        {loading || (updateInfo === undefined && !error) ? (
          <Stack spacing={1}>
            <Typography variant="body2">
              {t('traffic.checkingUpdates')}
            </Typography>
            <LinearProgress aria-label={t('traffic.checkingUpdates')} />
          </Stack>
        ) : error ? (
          <Alert severity="error">{t('traffic.updateCheckFailed')}</Alert>
        ) : updateInfo ? (
          <>
            <Alert severity={updateInfo.available ? 'success' : 'info'}>
              {updateInfo.available
                ? t('traffic.updateAvailable', {
                    version: `v${updateInfo.version}`,
                  })
                : t('traffic.upToDate')}
            </Alert>
            <Box
              tabIndex={0}
              role="region"
              aria-label={t('traffic.releaseNotes')}
              sx={{
                maxHeight: '30vh',
                overflow: 'auto',
                overflowWrap: 'anywhere',
                fontSize: 14,
                '& h1, & h2, & h3': { fontSize: 17 },
                '& ul, & ol': { pl: 3 },
              }}
            >
              <Suspense
                fallback={
                  <Typography variant="body2">
                    {t('traffic.releaseNotes')}
                  </Typography>
                }
              >
                <LazyReactMarkdown
                  skipHtml
                  components={{
                    a: ({ href, children }) => (
                      <Link
                        component="button"
                        onClick={() => href && openUrl(href)}
                        sx={{ verticalAlign: 'baseline' }}
                      >
                        {children}
                      </Link>
                    ),
                    img: () => null,
                  }}
                >
                  {updateInfo.body || t('traffic.noReleaseNotes')}
                </LazyReactMarkdown>
              </Suspense>
            </Box>
          </>
        ) : (
          <Alert severity="info">{t('traffic.noPublishedRelease')}</Alert>
        )}
      </Box>
      <Typography variant="body2" color="text.secondary">
        {t('traffic.manualUpdatesNote')}
      </Typography>
      <Stack
        direction="row"
        spacing={1}
        useFlexGap
        sx={{ mt: 2, flexWrap: 'wrap' }}
      >
        <Button
          variant="outlined"
          loading={loading}
          onClick={() => void refetch().catch(showNotice.error)}
        >
          {t('traffic.checkUpdates')}
        </Button>
        {!error && !loading && release?.available && (
          <Button
            variant="contained"
            onClick={() => openUrl(release.downloadUrl)}
          >
            {t('traffic.downloadPortable')}
          </Button>
        )}
        {releasesUrl && (
          <Button onClick={() => openUrl(release?.url ?? releasesUrl)}>
            {t('traffic.openReleases')}
          </Button>
        )}
      </Stack>
    </BaseDialog>
  )
}
