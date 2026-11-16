import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../', import.meta.url))
const { values } = parseArgs({ options: { ref: { type: 'string' } } })
const edition = JSON.parse(
  fs.readFileSync(path.join(root, 'traffic.edition.json'), 'utf8'),
)
const policy = JSON.parse(
  fs.readFileSync(path.join(root, 'docs/traffic/integration.json'), 'utf8'),
)
const git = (...args) =>
  execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim()
const baseline = edition.upstream.baseRevision
const target = git('rev-parse', '--verify', `${values.ref ?? 'HEAD'}^{commit}`)
const changed = git('diff', '--name-only', baseline, target)
  .split(/\r?\n/)
  .filter(Boolean)
const adapters = changed.filter((file) =>
  Object.keys(policy.adapters).some((pattern) =>
    path.matchesGlob(file, pattern),
  ),
)
const report = {
  baseline,
  target,
  changedFiles: changed.length,
  integrationFilesToReview: adapters.map((file) => ({
    file,
    reason: Object.entries(policy.adapters).find(([pattern]) =>
      path.matchesGlob(file, pattern),
    )[1],
  })),
}
console.log(JSON.stringify(report, null, 2))
