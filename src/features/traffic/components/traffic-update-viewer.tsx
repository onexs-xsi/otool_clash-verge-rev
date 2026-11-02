import { Box, Button, Typography } from '@mui/material'
import { type Ref, useImperativeHandle, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog, type DialogRef } from '@/components/base'
import { openExternalUrl } from '@/utils/open-external-url'

import {
  trafficDisplayName,
  trafficEdition,
  trafficReleasesUrl,
} from '../edition'

export function TrafficUpdateViewer({ ref }: { ref?: Ref<DialogRef> }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const releaseUrl = trafficReleasesUrl
  useImperativeHandle(ref, () => ({
    open: () => setOpen(true),
    close: () => setOpen(false),
  }))
  return (
    <BaseDialog
      open={open}
      title={t('traffic.manualUpdates')}
      contentSx={{ width: 450, maxWidth: '100%' }}
      disableCancel
      okBtn={t('traffic.close')}
      onClose={() => setOpen(false)}
      onOk={() => setOpen(false)}
    >
      <Typography variant="subtitle1" sx={{ mb: 1 }}>
        {trafficDisplayName} {trafficEdition}
      </Typography>
      <Typography variant="body2">{t('traffic.manualUpdatesNote')}</Typography>
      {releaseUrl && (
        <Box sx={{ mt: 2 }}>
          <Button onClick={() => void openExternalUrl(releaseUrl)}>
            {t('traffic.openReleases')}
          </Button>
        </Box>
      )}
    </BaseDialog>
  )
}
