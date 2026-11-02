export function applicationIdentity(
  app: string,
  path: string,
): [string, string] {
  if (app.startsWith('WSL:')) return [app, path]
  const original = path.trim().replace(/^"|"$/g, '')
  const normalized = original
    .replaceAll('/', '\\')
    .replace(/^(?:\\\\\?\\|\\\?\?\\)/, '')
  if (!/^(?:[a-z]:|\\\\)/i.test(normalized))
    return [/\.exe$/i.test(app) ? app.toLowerCase() : app, original]
  const parts = normalized.toLowerCase().split('\\')
  const version = /^\d+\.\d+\.\d+\.\d+$/
  for (let index = 1; index < parts.length; index += 1) {
    if (parts[index - 1] === 'windowsapps') {
      const fields = parts[index].match(
        /^(.+)_(\d+\.\d+\.\d+\.\d+)_(x64|x86|arm64|arm|neutral)_([^_]*)_([^_]+)$/,
      )
      if (fields) parts[index] = `${fields[1]}__${fields[5]}`
    } else if (
      parts.slice(index - 3, index).join('\\') === 'openai\\codex\\bin' &&
      parts[index + 1] === 'codex.exe' &&
      /^[0-9a-f]{7,64}$/.test(parts[index])
    )
      parts[index] = '@version'
    else if (parts[index - 1] === 'application' && version.test(parts[index]))
      parts[index] = '@version'
  }
  return [parts.at(-1) || app.toLowerCase(), parts.join('\\')]
}

export function normalizeApplicationKey(key: string) {
  try {
    const value: unknown = JSON.parse(key)
    if (
      Array.isArray(value) &&
      value.length === 2 &&
      value.every((item) => typeof item === 'string')
    )
      return JSON.stringify(applicationIdentity(value[0], value[1]))
  } catch {
    /* Older or external keys retain their original value. */
  }
  return key
}
