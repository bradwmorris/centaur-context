import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { readOverlay, stageComposition, publishOutput } from './context-ui.mjs';
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

test('second view is added solely in the overlay with deterministic order', async t => {
  const f = await fixture(t);
  await fs.cp(path.join(f.overlay, 'views/task-summary'), path.join(f.overlay, 'views/second'), { recursive: true });
  await edit(path.join(f.overlay, 'views/second/manifest.json'), m => { m.id = 'example:second'; m.order = -1; });
  await edit(f.config, c => c.views.push('views/second/manifest.json'));
  const result = await readOverlay(f.config, sha);
  assert.deepEqual(result.views.map(v => v.id), ['example:second', 'example:task-summary']);
});

test('dev HTTP cannot serve unselected overlay files or import private host modules', { skip: process.env.CONTEXT_UI_INTEGRATION !== '1', timeout: 120000 }, async t => {
  const { spawn } = await import('node:child_process');
  const { createServer } = await import('node:net');
  const f = await fixture(t);
  const secret = path.join(f.overlay, 'unrelated.txt');
  await fs.writeFile(secret, 'context-fixture-secret-never-served');
  await fs.writeFile(path.join(f.overlay, '.env'), 'VITE_SECRET=context-fixture-secret-never-served');
  const listener = createServer(); await new Promise(resolve => listener.listen(0, '127.0.0.1', resolve));
  const port = listener.address().port; await new Promise(resolve => listener.close(resolve));
  // A type-only private import must be rejected too, not silently erased by TS.
  await fs.appendFile(path.join(f.overlay, 'views/task-summary/index.tsx'), '\nimport type { Task } from "../../../src/types";\n');
  const child = spawn(process.execPath, [path.join(root, 'scripts/context-ui.mjs'), 'dev', '--config', f.config, '--port', String(port)], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  let logs = ''; child.stdout.on('data', b => logs += b); child.stderr.on('data', b => logs += b);
  const exited = new Promise(resolve => child.on('exit', resolve));
  t.after(async () => { child.kill('SIGTERM'); await exited; });
  const base = `http://127.0.0.1:${port}`;
  for (let count = 0; count < 500 && !logs.includes('Local:'); count++) {
    if (child.exitCode !== null) assert.fail(logs);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  assert.match(logs, /Local:/);
  for (const file of [secret, path.join(f.overlay, '.env')]) {
    const response = await fetch(`${base}/@fs${file}`);
    assert.equal(response.status, 403);
    assert.doesNotMatch(await response.text(), /context-fixture-secret-never-served/);
  }
  const imported = await fetch(`${base}/.context-overlay/views/0/index.tsx`);
  assert.equal(imported.status, 500);
  assert.match(await imported.text(), /private|outside|public/);
  child.kill('SIGTERM'); await exited;
  await assert.rejects(fetch(base)); // Shutdown must stop the actual Vite process.
});


test('publishes a build into a new directory and refuses to merge into existing output', async t => {
  const f = await fixture(t); const source = path.join(f.temp, 'dist'); const dest = path.join(f.temp, 'output');
  await fs.mkdir(path.join(source, 'assets'), { recursive: true });
  await fs.writeFile(path.join(source, 'index.html'), 'synthetic build');
  await fs.writeFile(path.join(source, 'assets/app.js'), 'export const test = 1;');
  await publishOutput(source, dest);
  assert.equal(await fs.readFile(path.join(dest, 'index.html'), 'utf8'), 'synthetic build');
  await assert.rejects(publishOutput(source, dest), { code: 'EEXIST' });
});

test('rejects lockfile traversal and mismatched package identity before installation', async t => {
  const f = await fixture(t); const lock = path.join(f.overlay, 'package-lock.json');
  await edit(lock, l => l.packages['../outside/node_modules/bad'] = { resolved: 'https://registry.npmjs.org/bad/-/bad-1.0.0.tgz', integrity: 'sha512-test' });
  await assert.rejects(readOverlay(f.config, sha), /Unsafe lockfile package path/);
  await edit(lock, l => { delete l.packages['../outside/node_modules/bad']; l.version = 'file:../outside'; });
  await assert.rejects(readOverlay(f.config, sha), /identity drift/);
});


test('rejects local-path peer dependencies and incompatible React peers', async t => {
  const f = await fixture(t); const pkg = path.join(f.overlay, 'package.json');
  await edit(pkg, p => p.peerDependencies = { outside: 'file:../private' });
  await assert.rejects(readOverlay(f.config, sha), /Only host React/);
  await edit(pkg, p => p.peerDependencies = { react: '18.0.0' });
  await assert.rejects(readOverlay(f.config, sha), /peer must exactly match/);
});
