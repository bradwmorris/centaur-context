// Run with Node 22: node --experimental-strip-types scripts/object-ui-preview.mjs [web root] [port]
// Local synthetic evidence only. No database, credentials, or live Context service is used.
import { createServer as createHttpServer } from 'node:http';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createObjectFixtureApi } from '../web/src/test-fixtures/objectApi.ts';
const webRoot = resolve(process.argv[2] || 'web');
const api = createObjectFixtureApi();
const server = createHttpServer(async (req, res) => {
  try {
    let body = '';
    for await (const chunk of req) body += chunk;
    const response = await api(`http://synthetic.test${req.url}`, { method: req.method, ...(body ? { body } : {}) });
    res.writeHead(response.status, Object.fromEntries(response.headers));
    res.end(await response.text());
  } catch { res.writeHead(500); res.end('Synthetic fixture request failed'); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
process.chdir(webRoot);
const { createServer } = await import(pathToFileURL(resolve(webRoot, 'node_modules/vite/dist/node/index.js')));
const vite = await createServer({ root: webRoot, server: { host: '127.0.0.1', port: Number(process.argv[3] || 49128), strictPort: true, proxy: { '/api': `http://127.0.0.1:${server.address().port}` } } });
await vite.listen();
vite.printUrls();
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, async () => { await vite.close(); server.close(); process.exit(0); });
