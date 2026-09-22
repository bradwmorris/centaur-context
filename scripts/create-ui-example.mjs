#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = process.argv[2];
if (!output) throw new Error('Usage: node scripts/create-ui-example.mjs /new/overlay/directory');
await fs.mkdir(output); // Never overwrite an existing overlay.
await fs.cp(path.join(root, 'examples/task-view'), path.join(output, 'views/task-summary'), { recursive: true });
const write = (name, value) => fs.writeFile(path.join(output, name), JSON.stringify(value, null, 2) + '\n');
const pkg = { name: 'synthetic-context-views', version: '1.0.0', private: true };
await write('package.json', pkg);
await write('package-lock.json', { name: pkg.name, version: pkg.version, lockfileVersion: 3, requires: true, packages: { '': { name: pkg.name, version: pkg.version } } });
await write('views.config.json', { schemaVersion: 1, views: ['views/task-summary/manifest.json'] });
await write('views/task-summary/manifest.json', { schemaVersion: 1, id: 'example:task-summary', label: 'Summary', hostApiVersion: 1, coreRevision: execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), section: 'tasks', version: '1.0.0', entry: 'index.tsx', files: ['index.tsx', 'styles.module.css'], order: 100 });
console.log(`Created synthetic overlay at ${path.resolve(output)}`);
