export function createTrafficIconLoader(
  fetchIcon: (path: string, app: string) => Promise<string | null>,
  now = Date.now,
) {
  const cache = new Map<string, { icon: string | null; expires: number }>()
  const pending = new Map<string, Promise<string | null>>()
  const queue: Array<() => void> = []
  let running = 0
  const key = (path: string, app: string) =>
    JSON.stringify([
      path.replaceAll('/', '\\').toLowerCase(),
      app.toLowerCase(),
    ])
  const peek = (path: string, app: string) => {
    const entry = cache.get(key(path, app))
    return entry && entry.expires > now() ? entry.icon : undefined
  }
  const drain = () => {
    while (running < 4 && queue.length) {
      running += 1
      queue.shift()!()
    }
  }
  const get = (path: string, app: string) => {
    const id = key(path, app)
    const cached = peek(path, app)
    if (cached !== undefined) return Promise.resolve(cached)
    const existing = pending.get(id)
    if (existing) return existing
    const result = new Promise<string | null>((resolve) => {
      queue.push(() => {
        void Promise.resolve()
          .then(() => fetchIcon(path, app))
          .catch(() => null)
          .then((icon) => {
            if (cache.size >= 256) cache.delete(cache.keys().next().value!)
            cache.set(id, { icon, expires: icon ? Infinity : now() + 60_000 })
            pending.delete(id)
            resolve(icon)
          })
          .finally(() => {
            running -= 1
            drain()
          })
      })
    })
    pending.set(id, result)
    drain()
    return result
  }
  return { get, peek }
}
