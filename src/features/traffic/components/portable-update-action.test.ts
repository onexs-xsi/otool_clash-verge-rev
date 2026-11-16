import { beforeEach, expect, it, vi } from 'vitest'

import {
  cancelPortableUpdate,
  installPortableUpdate,
  preparePortableUpdate,
  type PortableUpdatePlan,
} from '@/services/traffic/portable-update'

import { PortableUpdateAction } from './portable-update-action'

const hooks = vi.hoisted(() => ({
  values: [] as unknown[],
  index: 0,
  generation: { current: 0 },
}))
vi.mock('react', () => ({
  useState: (initial: unknown) => {
    const index = hooks.index++
    if (!(index in hooks.values)) hooks.values[index] = initial
    return [
      hooks.values[index],
      (value: unknown) => {
        hooks.values[index] = value
      },
    ]
  },
  useRef: () => hooks.generation,
}))
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('@mui/material', () => ({
  Alert: 'alert',
  Box: 'div',
  Button: 'button',
  LinearProgress: 'progress',
  Stack: 'div',
  Typography: 'p',
}))
vi.mock('@/components/base', () => ({ BaseDialog: 'dialog' }))
vi.mock('@/services/mutate', () => ({
  mutate: async (invoke: () => Promise<unknown>) => ({
    ok: true,
    value: await invoke(),
  }),
}))
vi.mock('@/services/notice-service', () => ({
  errorDetail: (failure: { detail: string }) => failure.detail,
}))
vi.mock('@/services/traffic/portable-update', () => ({
  preparePortableUpdate: vi.fn(),
  cancelPortableUpdate: vi.fn(),
  installPortableUpdate: vi.fn(),
}))

type Node = {
  type: unknown
  props: { children?: unknown; [key: string]: unknown }
}
const nodes = (value: unknown): Node[] => {
  if (Array.isArray(value)) return value.flatMap(nodes)
  if (value && typeof value === 'object' && 'props' in value) {
    const node = value as Node
    return [node, ...nodes(node.props.children)]
  }
  return []
}
const render = () => {
  hooks.index = 0
  return nodes(PortableUpdateAction({ version: '2.6.0-otool.15' }))
}
const dialog = () => render().find((node) => node.type === 'dialog')!.props
const click = (props: Node['props'], action: string) => {
  ;(props[action] as () => void)()
}
const begin = () =>
  click(
    render().find((node) => node.props.children === 'traffic.replaceInstall')!
      .props,
    'onClick',
  )
const plan: PortableUpdatePlan = {
  id: 'prepared-update',
  version: '2.6.0-otool.15',
  currentExecutable: "D:\\Tools\\中文 ' portable\\clash-verge.exe",
  sourceExecutable: 'C:\\Temp\\otool-update-fixture\\package\\clash-verge.exe',
  backupDirectory: 'D:\\Tools\\.otool-backups\\v14',
  downloadUrl:
    'https://github.com/onexs-xsi/otool_clash-verge-rev/releases/download/v2.6.0-otool.15/package.zip',
  packageSha256: 'a'.repeat(64),
}

beforeEach(() => {
  vi.clearAllMocks()
  hooks.values = []
  hooks.index = 0
  hooks.generation.current = 0
  vi.mocked(preparePortableUpdate).mockResolvedValue(plan)
  vi.mocked(cancelPortableUpdate).mockResolvedValue(undefined)
  vi.mocked(installPortableUpdate).mockResolvedValue(null)
})

it('only prepares on the first click, then displays both exact paths before execution', async () => {
  begin()
  await vi.waitFor(() =>
    expect(dialog().title).toBe('traffic.replaceConfirmTitle'),
  )
  const content = JSON.stringify(render())
  expect(content).toContain(JSON.stringify(plan.currentExecutable).slice(1, -1))
  expect(content).toContain(JSON.stringify(plan.sourceExecutable).slice(1, -1))
  expect(content).toContain(plan.downloadUrl)
  expect(installPortableUpdate).not.toHaveBeenCalled()
  click(dialog(), 'onCancel')
  await vi.waitFor(() => expect(dialog().open).toBe(false))
  expect(cancelPortableUpdate).toHaveBeenCalledOnce()
  expect(installPortableUpdate).not.toHaveBeenCalled()
})

it('starts the helper only after the second confirmation, locks pending actions and permits authorization retry', async () => {
  begin()
  await vi.waitFor(() =>
    expect(dialog().title).toBe('traffic.replaceConfirmTitle'),
  )
  let reject!: (error: unknown) => void
  vi.mocked(installPortableUpdate).mockImplementationOnce(
    () =>
      new Promise((_, rejectPromise) => {
        reject = rejectPromise
      }),
  )
  click(dialog(), 'onOk')
  expect(installPortableUpdate).toHaveBeenCalledWith(plan.id)
  expect(dialog().loading).toBe(true)
  expect(dialog().cancelDisabled).toBe(true)
  click(dialog(), 'onClose')
  expect(cancelPortableUpdate).not.toHaveBeenCalled()
  reject({ detail: 'Windows authorization was cancelled' })
  await vi.waitFor(() => expect(dialog().loading).toBe(false))
  expect(dialog().open).toBe(true)
  expect(JSON.stringify(render())).toContain(
    'Windows authorization was cancelled',
  )
  click(dialog(), 'onOk')
  expect(installPortableUpdate).toHaveBeenCalledTimes(2)
})

it('keeps the program open and offers a fresh download after verification failure', async () => {
  vi.mocked(preparePortableUpdate).mockRejectedValueOnce({
    detail: 'SHA-256 mismatch',
  })
  begin()
  await vi.waitFor(() => expect(dialog().okBtn).toBe('traffic.retryUpdate'))
  expect(JSON.stringify(render())).toContain('SHA-256 mismatch')
  expect(installPortableUpdate).not.toHaveBeenCalled()
  click(dialog(), 'onOk')
  await vi.waitFor(() =>
    expect(dialog().title).toBe('traffic.replaceConfirmTitle'),
  )
  expect(preparePortableUpdate).toHaveBeenCalledTimes(2)
  expect(installPortableUpdate).not.toHaveBeenCalled()
})

it('cancels preparation and ignores a late progress event or completed old plan', async () => {
  let resolve!: (value: PortableUpdatePlan) => void
  vi.mocked(preparePortableUpdate).mockImplementationOnce(
    () =>
      new Promise((resolvePromise) => {
        resolve = resolvePromise
      }),
  )
  begin()
  const onProgress = vi.mocked(preparePortableUpdate).mock.calls[0][1]
  onProgress({ stage: 'downloading', downloaded: 50, total: 100 })
  expect(render().find((node) => node.type === 'progress')?.props.value).toBe(
    50,
  )
  click(dialog(), 'onCancel')
  await vi.waitFor(() => expect(dialog().open).toBe(false))
  onProgress({ stage: 'extracting', downloaded: 100, total: 100 })
  resolve(plan)
  await Promise.resolve()
  await Promise.resolve()
  expect(dialog().open).toBe(false)
  expect(installPortableUpdate).not.toHaveBeenCalled()
})
