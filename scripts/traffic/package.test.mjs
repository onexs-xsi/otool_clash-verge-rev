import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

test('portable packaging keeps versioned archives and rejects overwrites or incomplete releases', {
  skip: process.platform !== 'win32' || process.arch !== 'x64',
}, () => {
  const temporaryRoot = fs.realpathSync(os.tmpdir())
  const directory = fs.mkdtempSync(
    path.join(temporaryRoot, 'clash-traffic-package-'),
  )
  try {
    for (const dir of [
      'scripts/traffic',
      'target/release',
      'src-tauri/sidecar',
      'src-tauri/resources',
      'docs/traffic',
    ])
      fs.mkdirSync(path.join(directory, dir), { recursive: true })
    fs.copyFileSync(
      fileURLToPath(new URL('./package.mjs', import.meta.url)),
      path.join(directory, 'scripts/traffic/package.mjs'),
    )
    const setVersion = (edition) => {
      const version = `2.6.0-otool.${edition}`
      fs.writeFileSync(
        path.join(directory, 'traffic.edition.json'),
        JSON.stringify({ edition, upstream: { version: '2.6.0' } }),
      )
      for (const name of ['package.json', 'src-tauri/tauri.conf.json']) {
        fs.writeFileSync(
          path.join(directory, name),
          JSON.stringify({ version }),
        )
      }
      fs.writeFileSync(
        path.join(directory, 'src-tauri/Cargo.toml'),
        `[package]\nname = "clash-verge"\nversion = "${version}"\n`,
      )
      fs.writeFileSync(
        path.join(directory, 'Cargo.lock'),
        `[[package]]\nname = "clash-verge"\nversion = "${version}"\n`,
      )
    }
    setVersion(1)
    const exe = Buffer.alloc(256)
    exe.write('MZ')
    exe.writeUInt32LE(0x80, 0x3c)
    exe.writeUInt32LE(0x4550, 0x80)
    exe.writeUInt16LE(0x8664, 0x84)
    fs.writeFileSync(
      path.join(directory, 'target/release/clash-verge.exe'),
      exe,
    )
    for (const name of ['verge-mihomo', 'verge-mihomo-alpha'])
      fs.writeFileSync(
        path.join(
          directory,
          `src-tauri/sidecar/${name}-x86_64-pc-windows-msvc.exe`,
        ),
        'fixture',
      )
    fs.writeFileSync(
      path.join(directory, 'src-tauri/resources/geoip.dat'),
      'fixture',
    )
    fs.writeFileSync(path.join(directory, 'LICENSE'), 'fixture license')
    fs.writeFileSync(
      path.join(directory, 'docs/traffic/portable-notes.txt'),
      '{{APP_VERSION}} {{TRAFFIC_VERSION}}',
    )
    const run = (version) =>
      spawnSync(
        process.execPath,
        [
          path.join(directory, 'scripts/traffic/package.mjs'),
          '--version',
          String(version),
          '--skip-build',
        ],
        { encoding: 'utf8' },
      )
    const first = run(1)
    assert.equal(first.status, 0, first.stderr)
    const original = fs.readFileSync(
      path.join(directory, 'releases/archive/v1/release.json'),
    )
    setVersion(2)
    assert.equal(run(2).status, 0)
    assert.equal(run(2).status, 1)
    assert.deepEqual(
      fs.readFileSync(path.join(directory, 'releases/archive/v1/release.json')),
      original,
    )
    assert.equal(
      fs.readFileSync(
        path.join(directory, 'releases/archive/v2/使用说明.txt'),
        'utf8',
      ),
      '2.6.0-otool.2 v2',
    )
    const manifest = JSON.parse(original.toString())
    assert.equal(manifest.releaseTag, 'v2.6.0-otool.1')
    assert.equal(manifest.upstreamVersion, '2.6.0')
    setVersion(3)
    fs.writeFileSync(
      path.join(directory, 'src-tauri/tauri.conf.json'),
      JSON.stringify({ version: '2.6.0' }),
    )
    assert.notEqual(run(3).status, 0)
    assert.ok(!fs.existsSync(path.join(directory, 'releases/archive/v3')))
    setVersion(3)
    fs.unlinkSync(path.join(directory, 'docs/traffic/portable-notes.txt'))
    assert.notEqual(run(3).status, 0)
    assert.ok(!fs.existsSync(path.join(directory, 'releases/archive/v3')))
    assert.ok(!fs.existsSync(path.join(directory, 'releases/current')))
    assert.ok(
      !fs
        .readdirSync(path.join(directory, 'target'))
        .some((name) => name.startsWith('portable-')),
    )
  } finally {
    assert.equal(path.dirname(directory), temporaryRoot)
    fs.rmSync(directory, { recursive: true, force: true })
  }
})
