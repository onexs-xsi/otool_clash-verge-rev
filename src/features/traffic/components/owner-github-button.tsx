import { Avatar, IconButton } from '@mui/material'
import { useTranslation } from 'react-i18next'

import { showNotice } from '@/services/notice-service'
import { openExternalUrl } from '@/utils/open-external-url'

import ownerAvatar from '../assets/onexs-xsi.png?inline'
import { trafficRepositoryUrl } from '../edition'

export function OwnerGithubButton() {
  const { t } = useTranslation()
  const repositoryUrl = trafficRepositoryUrl
  if (!repositoryUrl) return null
  const label = t('traffic.ownerGithub')
  return (
    <IconButton
      size="medium"
      color="inherit"
      title={label}
      aria-label={label}
      onClick={() => {
        void openExternalUrl(repositoryUrl).catch(showNotice.error)
      }}
    >
      <Avatar src={ownerAvatar} alt="" sx={{ width: 24, height: 24 }} />
    </IconButton>
  )
}
