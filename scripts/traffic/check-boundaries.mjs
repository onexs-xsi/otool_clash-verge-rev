import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../', import.meta.url))
const read = (name) => fs.readFileSync(path.join(root, name), 'utf8')
const policy = JSON.parse(read('docs/traffic/integration.json'))
const edition = JSON.parse(read('traffic.edition.json'))
const git = (...args) =>
  execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim()
const lines = (value) => value.split(/\r?\n/).filter(Boolean)
const matches = (file, patterns) =>
  patterns.some((pattern) => path.matchesGlob(file, pattern))

assert.equal(
  edition.automaticApplicationUpdates,
  false,
  'Signed fork application updates are not configured; keep portable updates manual.',
)
assert.match(edition.upstream.baseRevision, /^[a-f0-9]{40}$/)
assert.ok(
  edition.repository === null ||
    /^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/?$/.test(edition.repository),
  'Set repository to a real fork URL or null.',
)
git('cat-file', '-e', `${edition.upstream.baseRevision}^{commit}`)
const files = [
  ...new Set([
    ...lines(
      git('diff', '--name-only', '--no-renames', edition.upstream.baseRevision),
    ),
    ...lines(git('ls-files', '--others', '--exclude-standard')),
  ]),
]
const unexpected = files.filter(
  (file) =>
    !matches(file, policy.owned) &&
    !matches(file, Object.keys(policy.adapters)),
)
assert.deepEqual(
  unexpected,
  [],
  `Changes outside the documented integration surface: ${unexpected.join(', ')}`,
)

for (const folder of ['src/features/traffic', 'src/services/traffic']) {
  for (const entry of fs.readdirSync(path.join(root, folder), {
    recursive: true,
    withFileTypes: true,
  })) {
    if (!entry.isFile() || !/\.tsx?$/.test(entry.name)) continue
    const absolute = path.join(entry.parentPath, entry.name)
    const text = fs.readFileSync(absolute, 'utf8')
    assert.ok(!text.includes('@/components/traffic/'), absolute)
    assert.ok(!text.includes('connections.trafficStats.'), absolute)
  }
}
for (const locale of ['zh', 'zhtw']) {
  const expected = Object.keys(
    JSON.parse(read('src/locales/en/traffic.json')),
  ).sort()
  assert.deepEqual(
    Object.keys(JSON.parse(read(`src/locales/${locale}/traffic.json`))).sort(),
    expected,
    `${locale} traffic translations`,
  )
}
assert.ok(
  !read('src/services/cmds.ts').includes('get_traffic_statistics'),
  'Traffic RPC belongs to src/services/traffic/api.ts.',
)
assert.ok(
  !read('src-tauri/src/core/mod.rs').includes('mod traffic_database'),
  'Native implementation belongs to core/traffic/.',
)
assert.ok(
  read('src-tauri/src/core/traffic/edition.rs').includes(
    'traffic.edition.json',
  ),
)
assert.ok(read('scripts/traffic/package.mjs').includes("'--no-bundle'"))
assert.ok(
  !read('scripts/traffic/package.mjs').includes(
    "path.join(releases, 'current')",
  ),
)
console.log(
  `[traffic] boundaries valid: ${files.length} feature/integration files, manual application updates, dedicated RPC and locale owners.`,
)
