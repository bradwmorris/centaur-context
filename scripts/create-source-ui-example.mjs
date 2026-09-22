#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
const root = path.resolve(import.meta.dirname, '..');
const output = process.argv[2];
if (!output) throw new Error('Usage: create-source-ui-example.mjs /new/external/overlay');
await fs.mkdir(output);
const write = (file, value) => fs.writeFile(path.join(output, file), JSON.stringify(value, null, 2) + '\n');
const pkg = { name: 'context-sources-overlay', version: '1.0.0', private: true };
await write('package.json', pkg);
await write('package-lock.json', { name: pkg.name, version: pkg.version, lockfileVersion: 3, requires: true, packages: { '': { name: pkg.name, version: pkg.version } } });
const coreRevision = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
for (const [layout, label, order] of [['grid', 'Grid', 100], ['board', 'Kanban', 200]]) {
  const dest = path.join(output, 'views', layout);
  await fs.mkdir(dest, { recursive: true });
  for (const file of ['View.tsx', 'styles.module.css']) await fs.copyFile(path.join(root, 'examples/source-views', file), path.join(dest, file));
  await fs.writeFile(path.join(dest, 'index.tsx'), `import type { SourceViewProps } from '@centaur-context/ui';\nimport View from './View';\nexport default function Sources(props: SourceViewProps) { return <View {...props} layout="${layout}" />; }\n`);
  await write(`views/${layout}/manifest.json`, { schemaVersion: 1, hostApiVersion: 1, coreRevision, id: `sources:${layout}`, section: 'sources', label, version: '1.0.0', order, entry: 'index.tsx', files: ['index.tsx', 'View.tsx', 'styles.module.css'] });
}
await write('views.config.json', { schemaVersion: 1, views: ['views/grid/manifest.json', 'views/board/manifest.json'] });
console.log(`Created independent Sources overlay: ${path.resolve(output)}`);
