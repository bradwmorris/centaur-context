import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { readOverlay, stageComposition } from './context-ui.mjs';
const root = path.resolve(import.meta.dirname, '..');
const sha = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
async function fixture(t) {
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'context-view-test-'));
  t.after(() => fs.rm(temp, { recursive: true, force: true }));
  const overlay = path.join(temp, 'overlay');
  execFileSync(process.execPath, [path.join(root, 'scripts/create-ui-example.mjs'), overlay]);
  return { temp, overlay, config: path.join(overlay, 'views.config.json'), manifest: path.join(overlay, 'views/task-summary/manifest.json') };
}
async function edit(file, change) { const value = JSON.parse(await fs.readFile(file)); change(value); await fs.writeFile(file, JSON.stringify(value)); }
test('stages only explicitly selected sources, preserves both checkouts and records hashes', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.overlay, 'secret.txt'), 'DO_NOT_SERVE');
  await fs.writeFile(path.join(f.overlay, '.env'), 'TOKEN=DO_NOT_SERVE');
  const before = execFileSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' });
  const staged = await stageComposition({ config: f.config });
  t.after(() => fs.rm(staged.stage, { recursive: true, force: true }));
  assert.equal(staged.metadata.views[0].id, 'example:task-summary');
  assert.equal(staged.metadata.views[0].sourceHashes.length, 2);
  assert.match(await fs.readFile(path.join(staged.web, 'src/modules/externalViews.ts'), 'utf8'), /example:task-summary/);
  await assert.rejects(fs.access(path.join(staged.web, '.context-overlay/secret.txt')));
  await assert.rejects(fs.access(path.join(staged.web, '.context-overlay/.env')));
  assert.equal(execFileSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' }), before);
});
for (const [name, change, expected] of [
  ['wrong host', m => m.coreRevision = '0'.repeat(40), /coreRevision/],
  ['unsupported section', m => m.section = 'sources', /only schema/],
  ['unknown field', m => m.script = 'anything', /unknown field/],
  ['reserved ID', m => m.id = 'kanban', /invalid, reserved/],
  ['missing entry', m => m.entry = 'missing.tsx', /entry must appear/],
  ['escaping path', m => m.files.push('../outside.tsx'), /Unsafe selected/],
  ['secret file', m => m.files.push('.env'), /unsupported source/],
  ['duplicate source', m => m.files.push('index.tsx'), /duplicate/],
  ['unsupported API', m => m.hostApiVersion = 2, /only schema/],
]) test(name, async t => { const f = await fixture(t); await edit(f.manifest, change); await assert.rejects(readOverlay(f.config, sha), expected); });
test('rejects duplicate views and escaping symlinks', async t => {
  const f = await fixture(t);
  await edit(f.config, c => c.views.push(c.views[0]));
  await assert.rejects(readOverlay(f.config, sha), /duplicate/);
  await edit(f.config, c => c.views.pop());
  await fs.writeFile(path.join(f.temp, 'outside.tsx'), 'export default null');
  await fs.unlink(path.join(f.overlay, 'views/task-summary/index.tsx'));
  await fs.symlink(path.join(f.temp, 'outside.tsx'), path.join(f.overlay, 'views/task-summary/index.tsx'));
  await assert.rejects(readOverlay(f.config, sha), /escapes root/);
});
test('unlisted broken views are never read; stock composition removes external registry', async t => {
  const f = await fixture(t); await edit(f.config, c => c.views = []);
  await fs.writeFile(f.manifest, 'broken');
  assert.equal((await readOverlay(f.config, sha)).views.length, 0);
  const staged = await stageComposition(); t.after(() => fs.rm(staged.stage, { recursive: true, force: true }));
  assert.deepEqual(staged.metadata.views, []);
  assert.match(await fs.readFile(path.join(staged.web, 'src/modules/externalViews.ts'), 'utf8'), /= \[\]/);
});
test('rejects unlocked dependencies and dependency install scripts', async t => {
  const f = await fixture(t); const lock = path.join(f.overlay, 'package-lock.json');
  await edit(lock, l => l.packages['node_modules/bad'] = { resolved: 'file:../private', version: '1.0.0' });
  await assert.rejects(readOverlay(f.config, sha), /Unsupported non-registry/);
  await edit(lock, l => l.packages['node_modules/bad'] = { resolved: 'https://registry.npmjs.org/bad/-/bad-1.0.0.tgz', integrity: 'sha512-test', hasInstallScript: true });
  await assert.rejects(readOverlay(f.config, sha), /lifecycle script/);
});
