/**
 * Low-level R2 storage abstraction.
 *
 * Key order in the SYNC_BUCKET:
 *   notes/{id}.json        - Note payload
 *   embeddings/{id}.bin    - raw f32 bytes  (384 * 4 bytes)
 *   positions/{id}.json    - Position payload
 *   manifest.json          - latest snapshot summary
 *   snapshots/{id}.json    - historical snapshots
 */

const NOTES_PREFIX = 'notes/';
const EMBEDDINGS_PREFIX = 'embeddings/';
const POSITIONS_PREFIX = 'positions/';
const MANIFEST_KEY = 'manifest.json';
const SNAPSHOTS_PREFIX = 'snapshots/';

export function noteKey(id) {
  return `${NOTES_PREFIX}${id}.json`;
}

export function embeddingKey(id) {
  return `${EMBEDDINGS_PREFIX}${id}.bin`;
}

export function positionKey(id) {
  return `${POSITIONS_PREFIX}${id}.json`;
}

export function snapshotKey(id) {
  return `${SNAPSHOTS_PREFIX}${id}.json`;
}

/** Fetch a single JSON object from R2. Returns null on miss. */
export async function getJson(bucket, key) {
  const obj = await bucket.get(key);
  if (obj === null) return null;
  return obj.json();
}

/** Store a single JSON object in R2. */
export async function putJson(bucket, key, data) {
  await bucket.put(key, JSON.stringify(data), {
    httpMetadata: { contentType: 'application/json' },
  });
}

/** Fetch a raw binary blob from R2. Returns null on miss. */
export async function getBinary(bucket, key) {
  const obj = await bucket.get(key);
  if (obj === null) return null;
  return obj.arrayBuffer();
}

/** Store a raw binary blob in R2. */
export async function putBinary(bucket, key, data) {
  await bucket.put(key, data, {
    httpMetadata: { contentType: 'application/octet-stream' },
  });
}

/** Delete a key from R2. */
export async function del(bucket, key) {
  await bucket.delete(key);
}

/** List all keys under a prefix, returned in batches. */
export async function listKeys(bucket, prefix) {
  const keys = [];
  let cursor;
  do {
    const result = await bucket.list({ prefix, cursor, limit: 1000 });
    keys.push(...result.objects.map((o) => o.key));
    cursor = result.cursor;
  } while (cursor);
  return keys;
}

/** Get the current manifest (or a default). */
export async function getManifest(bucket) {
  const m = await getJson(bucket, MANIFEST_KEY);
  return m ?? { noteCount: 0, lastModified: null, snapshotId: null };
}

/** Write the manifest. */
export async function putManifest(bucket, manifest) {
  await putJson(bucket, MANIFEST_KEY, manifest);
}
