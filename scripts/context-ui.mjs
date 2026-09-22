#!/usr/bin/env node
/** Trusted build-time composition. All generated inputs live outside both checkouts. */
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fail = message => { throw new Error(message); };
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const json = async file => JSON.parse(await fs.readFile(file, 'utf8'));
const writeJSON = (file, value) => fs.writeFile(file, JSON.stringify(value, null, 2) + '\n');
const inside = (base, file) => file !== base && path.relative(base, file) !== '..' && !path.relative(base, file).startsWith('..' + path.sep) && !path.isAbsolute(path.relative(base, file));
function fields(value, allowed, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail(`${label}: expected object`);
  for (const key of Object.keys(value)) if (!allowed.includes(key)) fail(`${label}: unknown field ${key}`);
}
async function selectedFile(base, relative) {
  if (typeof relative !== 'string' || !relative || path.isAbsolute(relative) || relative.includes('\\') || relative.split('/').some(p => !p || p === '..' || p.startsWith('.'))) fail(`Unsafe selected path: ${relative}`);
  const file = await fs.realpath(path.resolve(base, relative));
  if (!inside(await fs.realpath(base), file) || !(await fs.stat(file)).isFile()) fail(`Selected file escapes root or is not a file: ${relative}`);
  return file;
}
const git = (cwd, ...args) => execFileSync('git', ['-C', cwd, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();

export async function readOverlay(configPath, revision) {
  const configFile = await fs.realpath(configPath);
  const base = path.dirname(configFile);
  const config = await json(configFile);
  fields(config, ['schemaVersion', 'views'], configFile);
  if (config.schemaVersion !== 1 || !Array.isArray(config.views)) fail('Configuration requires schemaVersion 1 and a views array');
  const ids = new Set(['kanban']); const files = new Set(); const views = [];
  for (const manifestPath of config.views) {
    const manifestFile = await selectedFile(base, manifestPath);
    const manifest = await json(manifestFile);
    fields(manifest, ['schemaVersion', 'id', 'label', 'hostApiVersion', 'coreRevision', 'section', 'version', 'entry', 'files', 'order'], manifestPath);
    if (manifest.schemaVersion !== 1 || manifest.hostApiVersion !== 1 || manifest.section !== 'tasks') fail(`${manifestPath}: only schema/API 1 and section tasks are supported`);
    if (!/^[a-z][a-z0-9-]*:[a-z][a-z0-9-]*$/.test(manifest.id) || ids.has(manifest.id)) fail(`${manifestPath}: invalid, reserved or duplicate view ID`);
    ids.add(manifest.id);
    if (!/^[0-9a-f]{40}$/.test(manifest.coreRevision) || manifest.coreRevision !== revision) fail(`${manifestPath}: coreRevision must equal ${revision}`);
    if (typeof manifest.label !== 'string' || !manifest.label.trim() || manifest.label.length > 80 || !/^\d+\.\d+\.\d+$/.test(manifest.version) || !Number.isSafeInteger(manifest.order)) fail(`${manifestPath}: invalid label, version or order`);
    if (!Array.isArray(manifest.files) || !manifest.files.includes(manifest.entry)) fail(`${manifestPath}: entry must appear in files`);
    const sources = [];
    for (const relative of manifest.files) {
      if (!/\.(tsx?|jsx?|css|svg|png|jpg|jpeg|webp|woff2?)$/.test(relative)) fail(`${manifestPath}: unsupported source/asset ${relative}`);
      const file = await selectedFile(path.dirname(manifestFile), relative);
      if (!inside(base, file) || files.has(file)) fail(`${manifestPath}: duplicate or escaping file ${relative}`);
      files.add(file);
      const data = await fs.readFile(file);
      sources.push({ relative, data, sha256: hash(data) });
    }
    views.push({ ...manifest, sources });
  }
  views.sort((a, b) => a.order - b.order || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const packageFile = await selectedFile(base, 'package.json');
  const lockFile = await selectedFile(base, 'package-lock.json');
  const pkg = await json(packageFile); const lock = await json(lockFile);
  fields(pkg, ['name', 'version', 'private', 'type', 'description', 'license', 'dependencies', 'peerDependencies', 'scripts', 'workspaces', 'devDependencies', 'optionalDependencies'], 'overlay package.json');
  if (pkg.scripts && Object.keys(pkg.scripts).length) fail('Overlay package scripts are unsupported');
  if (pkg.workspaces || pkg.devDependencies && Object.keys(pkg.devDependencies).length || pkg.optionalDependencies && Object.keys(pkg.optionalDependencies).length) fail('Overlay workspaces/dev/optional dependencies are unsupported; use dependencies and host peers');
  if (lock.name !== pkg.name || lock.version !== pkg.version) fail('Overlay package/lock identity drift');
  if (lock.lockfileVersion !== 3 || !lock.packages?.['']) fail('Overlay requires npm lockfileVersion 3');
  if (JSON.stringify(Object.entries(pkg.dependencies ?? {}).sort()) !== JSON.stringify(Object.entries(lock.packages[''].dependencies ?? {}).sort())) fail('Overlay package/lock dependency drift');
  for (const spec of Object.values(pkg.dependencies ?? {})) if (typeof spec !== 'string' || /^(file:|link:|workspace:|git|https?:|\.|\/)/.test(spec)) fail('Only locked registry dependency specifications are supported');
  for (const name of Object.keys(pkg.peerDependencies ?? {})) if (!['react', 'react-dom'].includes(name)) fail('Only host React and React DOM peers are supported; declare other packages as locked dependencies');
  const host = await json(path.join(root, 'web/package.json'));
  for (const name of ['react', 'react-dom']) {
    if (pkg.dependencies?.[name]) fail(`${name} must be a host peer, not an overlay dependency`);
    if (pkg.peerDependencies?.[name] && pkg.peerDependencies[name] !== host.dependencies[name]) fail(`${name}: peer must exactly match host ${host.dependencies[name]}`);
  }
  for (const [name, record] of Object.entries(lock.packages)) {
    if (!name) continue;
    if (!name.startsWith('node_modules/') || name.includes('\\') || name.split('/').some(part => part === '..' || part === '.' || !part)) fail(`Unsafe lockfile package path: ${name}`);
    if (/(^|\/)node_modules\/(react|react-dom)$/.test(name) && record.version !== host.dependencies[name.endsWith('react-dom') ? 'react-dom' : 'react']) fail('Incompatible locked React peer');
    if (record.link || typeof record.resolved !== 'string' || !record.resolved.startsWith('https://registry.npmjs.org/') || !record.integrity) fail(`Unsupported non-registry or unlocked dependency: ${name}`);
    if (record.hasInstallScript) fail(`Dependency requires unsupported lifecycle script: ${name}`);
  }
  let sourceRevision = null;
  try { sourceRevision = git(base, 'rev-parse', 'HEAD'); } catch { /* A plain directory is supported. */ }
  return { views, pkg, lock, sourceRevision, lockHash: hash(await fs.readFile(lockFile)) };
}

export async function stageComposition({ config, coreRoot = root } = {}) {
  const revision = git(coreRoot, 'rev-parse', 'HEAD');
  const overlay = config ? await readOverlay(config, revision) : null;
  const stage = await fs.mkdtemp(path.join(os.tmpdir(), 'context-ui-'));
  try {
    // Git's allowlist excludes ignored credentials, caches, databases and build output.
    const names = execFileSync('git', ['-C', coreRoot, 'ls-files', '-z', '--cached'], { encoding: 'utf8' }).split('\0').filter(Boolean);
    for (const name of new Set(names)) {
      if (!['Dockerfile', 'Cargo.toml', 'Cargo.lock'].includes(name) && !['web/', 'src/', 'migrations/', 'contract/', 'scripts/'].some(prefix => name.startsWith(prefix))) continue;
      if (name.split('/').some(p => p.startsWith('.')) || name.endsWith('.log')) continue;
      const source = path.join(coreRoot, name);
      let st; try { st = await fs.lstat(source); } catch { continue; }
      if (!st.isFile()) continue;
      const target = path.join(stage, name);
      await fs.mkdir(path.dirname(target), { recursive: true }); await fs.copyFile(source, target);
    }
    const web = path.join(stage, 'web');
    const metadata = {
      coreRevision: revision, dirtyCore: Boolean(git(coreRoot, 'status', '--porcelain', '--untracked-files=normal')),
      schemaVersion: 1, hostApiVersion: 1, backendApi: 'v2',
      hostLockHash: hash(await fs.readFile(path.join(web, 'package-lock.json'))),
      overlayLockHash: overlay?.lockHash ?? null, overlayRevision: overlay?.sourceRevision ?? null,
      views: overlay?.views.map(({ sources, ...manifest }) => ({ ...manifest, sourceHashes: sources.map(({ relative, sha256 }) => ({ file: relative, sha256 })) })) ?? [],
    };
    if (overlay) {
      const extension = path.join(web, '.context-overlay');
      await fs.mkdir(extension, { recursive: true });
      await writeJSON(path.join(extension, 'package.json'), overlay.pkg);
      await writeJSON(path.join(extension, 'package-lock.json'), overlay.lock);
      const imports = []; const registrations = [];
      for (const [index, view] of overlay.views.entries()) {
        for (const file of view.sources) {
          const dest = path.join(extension, 'views', String(index), file.relative);
          await fs.mkdir(path.dirname(dest), { recursive: true }); await fs.writeFile(dest, file.data);
        }
        imports.push(`import View${index} from ${JSON.stringify(`../../.context-overlay/views/${index}/${view.entry.replace(/\.(tsx?|jsx?)$/, '')}`)};`);
        registrations.push(`{ id: ${JSON.stringify(view.id)}, label: ${JSON.stringify(view.label)}, section: "tasks", icon: "◇", render: props => createElement(View${index}, props) }`);
      }
      await fs.writeFile(path.join(web, 'src/modules/externalViews.ts'), `import { createElement } from "react";\nimport type { ContextUiModule } from "./moduleRegistry";\n${imports.join('\n')}\nexport const externalViews: ContextUiModule[] = [${registrations.join(',\n')}];\n`);
      const tsconfig = await json(path.join(web, 'tsconfig.app.json'));
      tsconfig.include.push('.context-overlay/views');
      tsconfig.compilerOptions.allowJs = true;
      tsconfig.compilerOptions.checkJs = true;
      tsconfig.compilerOptions.paths = { '@centaur-context/ui': ['./src/ui/index.ts'], 'react': ['./node_modules/@types/react'], 'react/*': ['./node_modules/@types/react/*'], 'react-dom': ['./node_modules/@types/react-dom'], 'react-dom/*': ['./node_modules/@types/react-dom/*'] };
      await writeJSON(path.join(web, 'tsconfig.app.json'), tsconfig);
    }
    await writeJSON(path.join(web, 'context-composition.json'), metadata);
    await fs.mkdir(path.join(web, 'public'), { recursive: true });
    await writeJSON(path.join(web, 'public/context-build.json'), metadata);
    return { stage, web, metadata };
  } catch (error) { await fs.rm(stage, { recursive: true, force: true }); throw error; }
}

function cleanEnv() {
  const result = {};
  for (const key of ['PATH', 'HOME', 'TMPDIR', 'SystemRoot', 'DOCKER_HOST', 'DOCKER_CONTEXT', 'CENTAUR_CONTEXT_DEV_API_TARGET']) if (process.env[key]) result[key] = process.env[key];
  return result;
}
async function run(program, args, cwd) {
  await new Promise((resolve, reject) => {
    const child = spawn(program, args, { cwd, stdio: 'inherit', env: cleanEnv(), detached: process.platform !== 'win32' });
    const stop = () => {
      try { if (process.platform === 'win32') child.kill('SIGTERM'); else process.kill(-child.pid, 'SIGTERM'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
    };
    process.on('SIGINT', stop); process.on('SIGTERM', stop);
    const cleanup = () => { process.off('SIGINT', stop); process.off('SIGTERM', stop); };
    child.on('error', error => { cleanup(); reject(error); });
    child.on('exit', code => { cleanup(); code === 0 ? resolve() : reject(new Error(`${program} exited ${code}`)); });
  });
}
export async function publishOutput(source, output) {
  // Reserve the destination atomically, then copy its children (Node 22 refuses
  // copying the root itself with errorOnExist even into our new empty directory).
  await fs.mkdir(output, { recursive: false });
  for (const name of await fs.readdir(source)) {
    await fs.cp(path.join(source, name), path.join(output, name), { recursive: true, errorOnExist: true, force: false });
  }
}

export async function main(args) {
  const command = args.shift(); const options = {};
  if (!['dev', 'build', 'image'].includes(command)) fail('Usage: context-ui.mjs dev|build|image [--config FILE] [--out-dir DIR | --tag IMAGE]');
  while (args.length) {
    const key = args.shift(); const value = args.shift();
    if (!['--config', '--out-dir', '--tag', '--port'].includes(key) || !value || value.startsWith('--') || options[key]) fail(`Invalid option ${key}`);
    options[key] = value;
  }
  if (command === 'build' && !options['--out-dir']) fail('build requires --out-dir');
  if (command === 'image' && (!options['--tag'] || !/^[a-zA-Z0-9][a-zA-Z0-9._/:@-]*$/.test(options['--tag']))) fail('image requires a valid --tag');
  if (options['--port'] && (command !== 'dev' || !/^\d+$/.test(options['--port']) || Number(options['--port']) < 1024 || Number(options['--port']) > 65535)) fail('--port must be a dev port between 1024 and 65535');
  if (options['--out-dir'] && command !== 'build' || options['--tag'] && command !== 'image') fail('Option is not applicable to this command');
  const output = options['--out-dir'] && path.resolve(options['--out-dir']);
  if (output) { try { await fs.lstat(output); fail('Output path already exists; choose a new directory'); } catch (e) { if (e.code !== 'ENOENT') throw e; } }
  const { stage, web, metadata } = await stageComposition({ config: options['--config'] });
  try {
    console.log(`Core ${metadata.coreRevision}${metadata.dirtyCore ? ' (DIRTY DEVELOPMENT BUILD)' : ''}; ${metadata.views.length} external views`);
    if (command === 'image') await run('docker', ['build', '--pull=false', '--tag', options['--tag'], stage], stage);
    else {
      await run('npm', ['ci', '--no-audit', '--no-fund'], web);
      if (options['--config']) await run('npm', ['ci', '--ignore-scripts', '--no-audit', '--no-fund'], path.join(web, '.context-overlay'));
      await run('npm', ['run', 'type-check'], web);
      if (command === 'dev') await run('npm', ['run', 'dev', '--', '--host', '127.0.0.1', '--strictPort', '--port', options['--port'] ?? '5173'], web);
      else { await run('npm', ['run', 'build'], web); await publishOutput(path.join(web, 'dist'), output); }
    }
  } finally { await fs.rm(stage, { recursive: true, force: true }); }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch(error => { console.error(`Context UI: ${error.message}`); process.exitCode = 1; });
}
