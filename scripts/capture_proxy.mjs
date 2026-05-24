// Minimal transparent HTTP proxy that logs all request headers and body
import http from 'http';
import https from 'https';
import { URL } from 'url';

const LISTEN_PORT = 9999;
const MAX_BODY_SIZE = 10 * 1024 * 1024; // 10 MB

const SENSITIVE_HEADERS = new Set([
    'authorization', 'x-api-key', 'cookie', 'set-cookie', 'proxy-authorization',
]);

function redactHeaders(headers) {
    const result = {};
    for (const [key, value] of Object.entries(headers)) {
        if (SENSITIVE_HEADERS.has(key.toLowerCase())) {
            result[key] = '[REDACTED]';
        } else {
            result[key] = value;
        }
    }
    return result;
}

const server = http.createServer(async (req, res) => {
    let body = '';
    let totalSize = 0;
    for await (const chunk of req) {
        totalSize += chunk.length;
        if (totalSize > MAX_BODY_SIZE) {
            res.writeHead(413, { 'Content-Type': 'text/plain' });
            res.end('Payload Too Large');
            return;
        }
        body += chunk;
    }

    const target = new URL(req.headers['x-target-url'] || `https://api.anthropic.com${req.url}`);

    console.log('\n=== REQUEST ===');
    console.log(`${req.method} ${target.href}`);
    console.log('--- Headers ---');
    const safeHeaders = redactHeaders(req.headers);
    for (const [key, value] of Object.entries(safeHeaders)) {
        if (key === 'x-target-url') continue;
        console.log(`  ${key}: ${value}`);
    }
    console.log('--- Body (first 500 chars) ---');
    try {
        const parsed = JSON.parse(body);
        // Log key body fields without the full system prompt
        const summary = { ...parsed };
        if (summary.system) summary.system = `[${summary.system.length} blocks, first: ${JSON.stringify(summary.system[0]).substring(0, 100)}...]`;
        if (summary.messages) summary.messages = `[${summary.messages.length} messages]`;
        if (summary.tools) summary.tools = `[${summary.tools.length} tools]`;
        console.log(JSON.stringify(summary, null, 2));
    } catch {
        console.log(body.substring(0, 500));
    }

    // Forward to target — strip sensitive headers before forwarding
    const headers = { ...req.headers };
    delete headers['x-target-url'];
    delete headers['host'];
    headers['host'] = target.host;
    for (const hdr of Object.keys(headers)) {
        if (SENSITIVE_HEADERS.has(hdr.toLowerCase())) {
            delete headers[hdr];
        }
    }

    const proxyReq = https.request(target, {
        method: req.method,
        headers,
    }, (proxyRes) => {
        console.log('\n=== RESPONSE ===');
        console.log(`Status: ${proxyRes.statusCode}`);
        console.log('--- Response Headers ---');
        const safeResponseHeaders = redactHeaders(proxyRes.headers);
        for (const [key, value] of Object.entries(safeResponseHeaders)) {
            console.log(`  ${key}: ${value}`);
        }
        res.writeHead(proxyRes.statusCode, proxyRes.headers);
        proxyRes.pipe(res);
    });

    proxyReq.on('error', (e) => {
        console.error('Proxy error:', e.message);
        res.writeHead(502);
        res.end('Proxy error');
    });

    proxyReq.write(body);
    proxyReq.end();
});

server.listen(LISTEN_PORT, () => {
    console.log(`Capture proxy listening on http://localhost:${LISTEN_PORT}`);
    console.log('Usage: claude -p --api-key YOUR_KEY "test"');
});
