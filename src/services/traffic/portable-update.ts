import { Channel } from '@tauri-apps/api/core'

import { commands } from '@/services/bindings'
import type { PortableUpdateProgress } from '@/services/bindings'

export type {
  PortableUpdatePlan,
  PortableUpdateProgress,
  PortableUpdateResult,
} from '@/services/bindings'

export function preparePortableUpdate(
  version: string,
  onProgress: (progress: PortableUpdateProgress) => void,
) {
  const channel = new Channel<PortableUpdateProgress>()
  channel.onmessage = onProgress
  return commands.preparePortableUpdate(version, channel)
}

export const cancelPortableUpdate = () => commands.cancelPortableUpdate()
export const installPortableUpdate = (id: string) =>
  commands.installPortableUpdate(id)
export const getPortableUpdateResult = () => commands.getPortableUpdateResult()
