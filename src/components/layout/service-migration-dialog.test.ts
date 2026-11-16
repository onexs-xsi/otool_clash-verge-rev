import { useState } from 'react'
import { beforeEach, expect, it, vi } from 'vitest'

import {
  getRuntimeState,
  installService,
  reinstallService,
  continueWithSidecar,
  restartCore,
  type RunState,
} from '@/services/cmds'
import { showNotice } from '@/services/notice-service'

import { ServiceMigrationDialog } from './service-migration-dialog'

const store = vi.hoisted(() => ({
  dispatch: vi.fn(),
  runState: null as RunState | null,
}))

vi.mock('react', () => ({ useState: vi.fn() }))
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('@mui/material', () => ({ Alert: 'div' }))
vi.mock('@/components/base', () => ({ BaseDialog: 'dialog' }))
vi.mock('@/store/app-store-context', () => ({
  useAppReads: () => ({ readRunState: getRuntimeState }),
  useRunState: () => store.runState,
}))
vi.mock('@/services/cmds', () => ({
  getRuntimeState: vi.fn(),
  installService: vi.fn(),
  reinstallService: vi.fn(),
  repairService: vi.fn(),
  continueWithSidecar: vi.fn(),
  restartCore: vi.fn(),
}))
vi.mock('@/services/notice-service', () => ({
  showNotice: { warning: vi.fn(), error: vi.fn(), success: vi.fn() },
}))

beforeEach(() => {
  vi.clearAllMocks()
  store.runState = null
  vi.mocked(useState)
    .mockReturnValueOnce([false, vi.fn()])
    .mockReturnValueOnce([false, vi.fn()])
    .mockReturnValueOnce([true, vi.fn()])
})

it.each([
  { service: 'ready', mode: 'NotRunning' },
  { service: 'unknown', mode: 'NotRunning' },
  { service: 'unavailable', mode: 'Service' },
  { service: 'unavailable', mode: 'NotRunning', opInFlight: true },
  { service: 'unavailable', mode: 'NotRunning', pendingAction: 'reinstall' },
] satisfies Partial<RunState>[])(
  'blocks Sidecar continuation from %j',
  (state) => {
    store.runState = state as unknown as RunState
    const dialog = ServiceMigrationDialog()
    expect(dialog.props.open).toBe(true)
    expect(
      Boolean(dialog.props.disableCancel || dialog.props.cancelDisabled),
    ).toBe(true)
  },
)

const startupMismatch: RunState = {
  mode: 'NotRunning',
  service: 'versionMismatch',
  serviceUnavailableReason: null,
  pendingAction: null,
  sidecarAllowed: false,
  isAdmin: false,
  opInFlight: false,
  serviceUsable: false,
  tunCapable: false,
  serviceNeedsAttention: true,
}

const startupDialog = (state: RunState = startupMismatch) => {
  vi.mocked(useState)
    .mockReset()
    .mockReturnValueOnce([false, vi.fn()])
    .mockReturnValueOnce([false, vi.fn()])
    .mockReturnValueOnce([false, vi.fn()])
  store.runState = state
  return ServiceMigrationDialog()
}

it('asks before updating an incompatible startup service', () => {
  const dialog = startupDialog()
  expect(dialog.props.open).toBe(true)
  expect(dialog.props.loading).toBe(false)
  expect(dialog.props.cancelDisabled).toBe(false)
  expect(dialog.props.disableCancel).toBe(false)
  expect(reinstallService).not.toHaveBeenCalled()
  expect(continueWithSidecar).not.toHaveBeenCalled()
  expect(restartCore).not.toHaveBeenCalled()
})

it.each([
  {
    ...startupMismatch,
    mode: 'Service',
    service: 'ready',
    serviceUsable: true,
    tunCapable: true,
    serviceNeedsAttention: false,
  },
  {
    ...startupMismatch,
    mode: 'Sidecar',
    service: 'notInstalled',
    sidecarAllowed: true,
    serviceNeedsAttention: false,
  },
] satisfies RunState[])(
  'does not prompt for an already usable startup state: %j',
  (state) => {
    expect(startupDialog(state).props.open).toBe(false)
    expect(reinstallService).not.toHaveBeenCalled()
    expect(continueWithSidecar).not.toHaveBeenCalled()
  },
)

it('offers repair for an unavailable installed service before issuing any command', () => {
  const dialog = startupDialog({
    ...startupMismatch,
    service: 'unavailable',
    serviceUnavailableReason: 'The registered service did not answer',
  })
  expect(dialog.props.open).toBe(true)
  expect(dialog.props.okBtn).toBe('layout.components.serviceMigration.repair')
  expect(reinstallService).not.toHaveBeenCalled()
  expect(restartCore).not.toHaveBeenCalled()
})

it('updates only after authorization and confirms service mode after restarting', async () => {
  const dialog = startupDialog()
  vi.mocked(reinstallService).mockResolvedValue({ status: 'installed' })
  vi.mocked(restartCore).mockResolvedValue(null)
  const ready: RunState = {
    ...startupMismatch,
    service: 'ready',
    serviceUsable: true,
    tunCapable: true,
    serviceNeedsAttention: false,
  }
  vi.mocked(getRuntimeState)
    .mockResolvedValueOnce(ready)
    .mockResolvedValueOnce({ ...ready, mode: 'Service' })

  dialog.props.onOk()
  await vi.waitFor(() => {
    expect(showNotice.success).toHaveBeenCalledWith(
      'layout.components.serviceMigration.success',
    )
  })
  expect(reinstallService).toHaveBeenCalledOnce()
  expect(restartCore).toHaveBeenCalledOnce()
  expect(continueWithSidecar).not.toHaveBeenCalled()
})

it('does not report success when the core remains stopped after an authorized update', async () => {
  const dialog = startupDialog()
  vi.mocked(reinstallService).mockResolvedValue({ status: 'installed' })
  vi.mocked(restartCore).mockResolvedValue(null)
  vi.mocked(getRuntimeState).mockResolvedValue(startupMismatch)

  dialog.props.onOk()
  await vi.waitFor(() => {
    expect(getRuntimeState).toHaveBeenCalledTimes(2)
  })
  expect(showNotice.success).not.toHaveBeenCalled()
  expect(showNotice.error).toHaveBeenCalledWith(
    'settings.sections.proxyControl.messages.installedCoreNotOnService',
  )
})

it('keeps the update dialog retryable when authorization is cancelled', async () => {
  const dialog = startupDialog()
  vi.mocked(reinstallService).mockRejectedValue(
    new Error('Authorization cancelled'),
  )
  vi.mocked(getRuntimeState).mockResolvedValue(startupMismatch)

  dialog.props.onOk()
  await vi.waitFor(() => {
    expect(showNotice.error).toHaveBeenCalledWith(
      'layout.components.serviceMigration.errors.actionFailed',
      expect.any(Error),
    )
  })
  expect(restartCore).not.toHaveBeenCalled()
  expect(showNotice.success).not.toHaveBeenCalled()
  expect(vi.mocked(useState).mock.results[2].value[1]).not.toHaveBeenCalledWith(
    false,
  )
})

it('uses Sidecar only after the user chooses it for this session', async () => {
  const dialog = startupDialog()
  vi.mocked(continueWithSidecar).mockResolvedValue(null)
  vi.mocked(getRuntimeState).mockResolvedValue({
    ...startupMismatch,
    mode: 'Sidecar',
    sidecarAllowed: true,
    serviceNeedsAttention: false,
  })

  expect(continueWithSidecar).not.toHaveBeenCalled()
  dialog.props.onCancel()
  await vi.waitFor(() => {
    expect(getRuntimeState).toHaveBeenCalledOnce()
  })
  expect(continueWithSidecar).toHaveBeenCalledOnce()
  expect(reinstallService).not.toHaveBeenCalled()
  expect(restartCore).not.toHaveBeenCalled()
})

it('reports the permission refusal and finishes after installation falls back to Sidecar', async () => {
  store.runState = {
    service: 'notInstalled',
    mode: 'NotRunning',
  } as unknown as RunState
  const reason = 'core path C:\\ is writable by Everyone'
  vi.mocked(installService).mockResolvedValue({ status: 'sidecar', reason })
  vi.mocked(getRuntimeState).mockResolvedValue({
    service: 'unavailable',
    mode: 'Sidecar',
    sidecarAllowed: true,
    serviceNeedsAttention: false,
  } as RunState)

  const dialog = ServiceMigrationDialog()
  dialog.props.onOk()
  await vi.waitFor(() => {
    expect(showNotice.warning).toHaveBeenCalledWith(
      'settings.feedback.notifications.clashService.permissionFallback',
      { reason },
      0,
    )
  })

  expect(restartCore).not.toHaveBeenCalled()
  expect(showNotice.error).not.toHaveBeenCalled()
  expect(showNotice.success).not.toHaveBeenCalled()
  expect(vi.mocked(useState).mock.results[2].value[1]).toHaveBeenLastCalledWith(
    false,
  )
})

it.each([
  { service: 'notInstalled', mode: 'NotRunning' },
  { service: 'versionMismatch', mode: 'NotRunning' },
  { service: 'unavailable', mode: 'NotRunning' },
  { service: 'notInstalled', mode: 'Sidecar', pendingAction: 'install' },
] satisfies Partial<RunState>[])(
  'offers Sidecar continuation from %j',
  (state) => {
    store.runState = state as unknown as RunState
    expect(ServiceMigrationDialog().props.disableCancel).toBe(false)
  },
)
