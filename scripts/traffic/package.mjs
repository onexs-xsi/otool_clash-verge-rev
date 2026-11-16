import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'

const root = path.resolve(import.meta.dirname, '../..')
const { values } = parseArgs({
  options: {
    version: { type: 'string' },
    'skip-build': { type: 'boolean', default: false },
  },
})
if (process.platform !== 'win32' || process.arch !== 'x64') {
  throw new Error('This portable package targets Windows x64.')
}
if (!/^[1-9]\d*$/.test(values.version ?? '')) {
  throw new Error('Usage: pnpm run portable --version <number> [--skip-build]')
}

const edition = `v${values.version}`
const editionConfig = JSON.parse(
  fs.readFileSync(path.join(root, 'traffic.edition.json'), 'utf8'),
)
const { version: appVersion } = JSON.parse(
  fs.readFileSync(path.join(root, 'package.json'), 'utf8'),
)
const expectedVersion = `${editionConfig.upstream.version}-otool.${editionConfig.edition}`
const tauriVersion = JSON.parse(
  fs.readFileSync(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8'),
).version
const cargoVersion = fs
  .readFileSync(path.join(root, 'src-tauri/Cargo.toml'), 'utf8')
  .match(/^version = "([^"]+)"/m)?.[1]
const lockedVersion = fs
  .readFileSync(path.join(root, 'Cargo.lock'), 'utf8')
  .match(/name = "clash-verge"\r?\nversion = "([^"]+)"/)?.[1]
if (
  Number(values.version) !== editionConfig.edition ||
  [appVersion, tauriVersion, cargoVersion, lockedVersion].some(
    (version) => version !== expectedVersion,
  )
) {
  throw new Error(
    `Synchronize the edition, package.json, Tauri and Cargo versions to ${expectedVersion} before packaging.`,
  )
}
const releases = path.join(root, 'releases')
const archive = path.join(releases, 'archive')
const destination = path.join(archive, edition)
if (fs.existsSync(destination)) {
  throw new Error(
    `Release already exists and will not be overwritten: ${destination}`,
  )
}
const existingEditions = fs.existsSync(archive)
  ? fs
      .readdirSync(archive, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && /^v[1-9]\d*$/.test(entry.name))
      .map((entry) => Number(entry.name.slice(1)))
  : []
const latestEdition = Math.max(0, ...existingEditions)
if (Number(values.version) <= latestEdition) {
  throw new Error(`The next edition must be greater than v${latestEdition}.`)
}
if (!values['skip-build']) {
  execFileSync(
    process.execPath,
    [
      path.join(root, 'node_modules/@tauri-apps/cli/tauri.js'),
      'build',
      '--no-bundle',
      '--ci',
      '--config',
      path.join(root, 'scripts/traffic/tauri.conf.json'),
    ],
    {
      cwd: root,
      stdio: 'inherit',
      env: process.env,
    },
  )
}

const build = path.join(root, 'target', 'release')
const appBytes = fs.readFileSync(path.join(build, 'clash-verge.exe'))
if (appBytes.toString('ascii', 0, 2) !== 'MZ') {
  throw new Error('The release application is not a Windows executable.')
}
const peOffset = appBytes.readUInt32LE(0x3c)
if (
  appBytes.readUInt32LE(peOffset) !== 0x4550 ||
  appBytes.readUInt16LE(peOffset + 4) !== 0x8664
) {
  throw new Error('The release application is not Windows x64.')
}

const staging = fs.mkdtempSync(path.join(root, 'target', 'portable-'))
if (path.dirname(staging) !== path.join(root, 'target')) {
  throw new Error('Refusing to clean a directory outside the build workspace.')
}
try {
  fs.writeFileSync(path.join(staging, 'clash-verge.exe'), appBytes)
  for (const core of ['verge-mihomo', 'verge-mihomo-alpha']) {
    fs.copyFileSync(
      path.join(
        root,
        'src-tauri',
        'sidecar',
        `${core}-x86_64-pc-windows-msvc.exe`,
      ),
      path.join(staging, `${core}.exe`),
    )
  }
  fs.cpSync(
    path.join(root, 'src-tauri', 'resources'),
    path.join(staging, 'resources'),
    { recursive: true },
  )
  fs.copyFileSync(path.join(root, 'LICENSE'), path.join(staging, 'LICENSE'))
  const notes = fs.readFileSync(
    path.join(root, 'docs/traffic/portable-notes.txt'),
    'utf8',
  )
  fs.writeFileSync(
    path.join(staging, '使用说明.txt'),
    notes
      .replaceAll('{{TRAFFIC_VERSION}}', edition)
      .replaceAll('{{APP_VERSION}}', appVersion),
    'utf8',
  )

  const hashes = {}
  const hashFiles = (directory) => {
    for (const entry of fs
      .readdirSync(directory, { withFileTypes: true })
      .sort((a, b) => a.name.localeCompare(b.name))) {
      const file = path.join(directory, entry.name)
      if (entry.isDirectory()) hashFiles(file)
      else
        hashes[path.relative(staging, file).split(path.sep).join('/')] =
          createHash('sha256').update(fs.readFileSync(file)).digest('hex')
    }
  }
  hashFiles(staging)
  fs.writeFileSync(
    path.join(staging, 'release.json'),
    `${JSON.stringify({ edition, appVersion, upstreamVersion: editionConfig.upstream.version, releaseTag: `v${appVersion}`, platform: 'windows-x64', packagedAt: new Date().toISOString(), sha256: hashes }, null, 2)}\n`,
  )

  fs.mkdirSync(archive, { recursive: true })
  if (fs.existsSync(destination)) {
    throw new Error(
      `Release already exists and will not be overwritten: ${destination}`,
    )
  }
  fs.renameSync(staging, destination)
  console.log(
    `Portable ${edition}: ${path.join(destination, 'clash-verge.exe')}`,
  )
} finally {
  fs.rmSync(staging, { recursive: true, force: true })
}
