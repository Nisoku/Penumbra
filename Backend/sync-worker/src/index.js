import { handlePush, handlePull, handleStatus, handleClear } from './sync.js';
import { noteKey, embeddingKey, positionKey, getJson, putJson, getBinary, putBinary } from './storage.js';

/**
 * Simple request router for the Penumbra sync Worker.
 *
 * Routes:
 *   POST /sync/push           - batch upload notes/embeddings/positions
 *   POST /sync/pull           - batch download
 *   GET  /sync/status         - storage status
 *   POST /sync/clear          - wipe all data
 *   PUT  /sync/notes/:id      - single note upload
 *   GET  /sync/notes/:id      - single note download
 *   PUT  /sync/embeddings/:id - single embedding upload
 *   GET  /sync/embeddings/:id - single embedding download
 *   PUT  /sync/positions/:id  - single position upload
 *   GET  /sync/positions/:id  - single position download
 */
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const bucket = env.SYNC_BUCKET;
    const method = request.method;
    const path = url.pathname;

    // CORS headers for browser clients
    const corsHeaders = {
      'access-control-allow-origin': '*',
      'access-control-allow-methods': 'GET, PUT, POST, DELETE, OPTIONS',
      'access-control-allow-headers': 'content-type',
    };

    if (method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    const route = `${method} ${path}`;

    try {
      let response;

      if (route === 'POST /sync/push') {
        response = await handlePush(request, bucket);
      } else if (route === 'POST /sync/pull') {
        response = await handlePull(request, bucket);
      } else if (route === 'GET /sync/status') {
        response = await handleStatus(request, bucket);
      } else if (route === 'POST /sync/clear') {
        response = await handleClear(request, bucket);
      } else if (path.startsWith('/sync/notes/') && method === 'PUT') {
        const id = path.replace('/sync/notes/', '');
        const body = await request.json();
        await putJson(bucket, noteKey(id), body);
        response = new Response(JSON.stringify({ id }), {
          headers: { 'content-type': 'application/json' },
        });
      } else if (path.startsWith('/sync/notes/') && method === 'GET') {
        const id = path.replace('/sync/notes/', '');
        const data = await getJson(bucket, noteKey(id));
        if (!data) {
          return new Response('not found', { status: 404, headers: corsHeaders });
        }
        response = new Response(JSON.stringify(data), {
          headers: { 'content-type': 'application/json' },
        });
      } else if (path.startsWith('/sync/embeddings/') && method === 'PUT') {
        const id = path.replace('/sync/embeddings/', '');
        const buf = await request.arrayBuffer();
        await putBinary(bucket, embeddingKey(id), buf);
        response = new Response(JSON.stringify({ id }), {
          headers: { 'content-type': 'application/json' },
        });
      } else if (path.startsWith('/sync/embeddings/') && method === 'GET') {
        const id = path.replace('/sync/embeddings/', '');
        const buf = await getBinary(bucket, embeddingKey(id));
        if (!buf) {
          return new Response('not found', { status: 404, headers: corsHeaders });
        }
        response = new Response(buf, {
          headers: { 'content-type': 'application/octet-stream' },
        });
      } else if (path.startsWith('/sync/positions/') && method === 'PUT') {
        const id = path.replace('/sync/positions/', '');
        const body = await request.json();
        await putJson(bucket, positionKey(id), body);
        response = new Response(JSON.stringify({ id }), {
          headers: { 'content-type': 'application/json' },
        });
      } else if (path.startsWith('/sync/positions/') && method === 'GET') {
        const id = path.replace('/sync/positions/', '');
        const data = await getJson(bucket, positionKey(id));
        if (!data) {
          return new Response('not found', { status: 404, headers: corsHeaders });
        }
        response = new Response(JSON.stringify(data), {
          headers: { 'content-type': 'application/json' },
        });
      } else {
        return new Response('not found', { status: 404, headers: corsHeaders });
      }

      // Attach CORS headers
      const finalHeaders = new Headers(response.headers);
      for (const [k, v] of Object.entries(corsHeaders)) {
        finalHeaders.set(k, v);
      }

      return new Response(response.body, {
        status: response.status,
        headers: finalHeaders,
      });
    } catch (err) {
      return new Response(
        JSON.stringify({ error: err.message }),
        {
          status: 500,
          headers: {
            'content-type': 'application/json',
            ...corsHeaders,
          },
        },
      );
    }
  },
};
