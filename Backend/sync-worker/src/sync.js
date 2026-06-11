import {
  noteKey,
  embeddingKey,
  positionKey,
  snapshotKey,
  getJson,
  putJson,
  getBinary,
  putBinary,
  del,
  listKeys,
  getManifest,
  putManifest,
} from './storage.js';

/**
 * Push a batch of changes from the client.
 *
 * Request body (JSON):
 *   {
 *     notes:        { [id]: { title, body, tags, ... } },
 *     embeddings:   { [id]: number[] },
 *     positions:    { [id]: { x, y } },
 *     snapshotId?: string
 *   }
 *
 * Returns:
 *   { accepted: number, snapshotId: string }
 */
export async function handlePush(request, bucket) {
  const body = await request.json();
  const { notes, embeddings, positions, snapshotId } = body;
  let accepted = 0;

  // Conflict detection: if client provides a snapshotId, it must match
  // the current server snapshot.  If they differ, another client has
  // pushed since this client's last pull.
  if (snapshotId) {
    const manifest = await getManifest(bucket);
    if (manifest.snapshotId && manifest.snapshotId !== snapshotId) {
      return new Response(
        JSON.stringify({
          error: 'conflict',
          currentSnapshotId: manifest.snapshotId,
          message: 'snapshotId mismatch, pull latest before pushing',
        }),
        {
          status: 409,
          headers: { 'content-type': 'application/json' },
        },
      );
    }
  }

  // Store notes
  if (notes) {
    for (const [id, data] of Object.entries(notes)) {
      await putJson(bucket, noteKey(id), data);
      accepted++;
    }
  }

  // Store embeddings as raw f32 bytes
  if (embeddings) {
    for (const [id, vec] of Object.entries(embeddings)) {
      const buf = new Float32Array(vec).buffer;
      await putBinary(bucket, embeddingKey(id), buf);
    }
  }

  // Store positions
  if (positions) {
    for (const [id, pos] of Object.entries(positions)) {
      await putJson(bucket, positionKey(id), pos);
    }
  }

  // Update manifest with a *new* snapshot id
  const manifest = await getManifest(bucket);
  const now = new Date().toISOString();
  const newSnapshotId = crypto.randomUUID();

  manifest.noteCount += accepted;
  manifest.lastModified = now;
  manifest.snapshotId = newSnapshotId;

  // Write a snapshot record
  await putJson(bucket, snapshotKey(newSnapshotId), {
    timestamp: now,
    noteCount: manifest.noteCount,
    snapshotId: newSnapshotId,
  });

  await putManifest(bucket, manifest);

  return new Response(
    JSON.stringify({ accepted, snapshotId: newSnapshotId }),
    { headers: { 'content-type': 'application/json' } },
  );
}

/**
 * Pull changes from the server, optionally since a given snapshot.
 *
 * Query params:
 *   ?since=<snapshotId>   - only return notes changed after this snapshot
 *   (omit for full dump)
 *
 * Returns:
 *   {
 *     notes:      { [id]: { title, body, ... } },
 *     embeddings: { [id]: number[] },
 *     positions:  { [id]: { x, y } },
 *     snapshot:   { snapshotId, timestamp, noteCount }
 *   }
 */
export async function handlePull(request, bucket) {
  const url = new URL(request.url);
  const since = url.searchParams.get('since');

  const manifest = await getManifest(bucket);

  // List all note keys
  const noteKeys = await listKeys(bucket, 'notes/');

  const notes = {};
  const embeddings = {};
  const positions = {};

  for (const key of noteKeys) {
    const id = key.replace('notes/', '').replace('.json', '');

    const note = await getJson(bucket, key);
    if (note) notes[id] = note;

    const embBuf = await getBinary(bucket, embeddingKey(id));
    if (embBuf) {
      embeddings[id] = Array.from(new Float32Array(embBuf));
    }

    const pos = await getJson(bucket, positionKey(id));
    if (pos) positions[id] = pos;
  }

  // Fetch latest snapshot for the response
  const snapshot = manifest.snapshotId
    ? await getJson(bucket, snapshotKey(manifest.snapshotId))
    : null;

  return new Response(
    JSON.stringify({ notes, embeddings, positions, snapshot }),
    { headers: { 'content-type': 'application/json' } },
  );
}

/**
 * Return storage status.
 *
 * Returns:
 *   { noteCount, lastModified, snapshotId, storageBytes }
 */
export async function handleStatus(request, bucket) {
  const manifest = await getManifest(bucket);

  // Approximate storage: sum all object sizes (cheap HEAD-like)
  let storageBytes = 0;
  const allKeys = await listKeys(bucket, '');
  for (const key of allKeys) {
    const obj = await bucket.head(key);
    if (obj) storageBytes += obj.size;
  }

  return new Response(
    JSON.stringify({
      noteCount: manifest.noteCount,
      lastModified: manifest.lastModified,
      snapshotId: manifest.snapshotId,
      storageBytes,
      storageLimit: 512 * 1024 * 1024, // 512 MB
    }),
    { headers: { 'content-type': 'application/json' } },
  );
}

/**
 * Delete all data (reset).
 */
export async function handleClear(request, bucket) {
  const allKeys = await listKeys(bucket, '');
  await Promise.all(allKeys.map((k) => del(bucket, k)));
  return new Response(
    JSON.stringify({ cleared: allKeys.length }),
    { headers: { 'content-type': 'application/json' } },
  );
}
