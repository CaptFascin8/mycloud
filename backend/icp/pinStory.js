'use strict';
// ============================================================================
//  pinStory.js — pin an anonymized Soul story to IPFS and return { story_cid,
//  story_hash }, where story_hash = sha256 of the raw UTF-8 story bytes,
//  lowercase hex (the SAME rule the canister verifies for LegalDoc — raw bytes,
//  no canonical-encoding question; trivially reproducible in any language).
//
//  Wiring to mycloud's Kubo node is via env:
//    IPFS_API_URL   e.g. http://127.0.0.1:5001  (Kubo RPC API /api/v0/add)
//    IPFS_API_AUTH  optional "Bearer ..." or basic creds if fronted by auth
//
//  If IPFS_API_URL is unset, pinStory runs in DRY mode: it computes the hash
//  and returns story_cid:null so archiving can proceed without a story while
//  the IPFS path is finalized with mycloud Claude.
// ============================================================================

const crypto = require('crypto');

function sha256HexUtf8(text) {
  return crypto.createHash('sha256').update(Buffer.from(text, 'utf8')).digest('hex');
}

/**
 * @param {string} storyText  anonymized story (already scrubbed by vetting)
 * @returns {Promise<{story_cid: string|null, story_hash: string}>}
 */
async function pinStory(storyText) {
  const story_hash = sha256HexUtf8(storyText);

  const apiUrl = process.env.IPFS_API_URL;
  if (!apiUrl) {
    console.warn('[pinStory] IPFS_API_URL unset — DRY mode, returning hash only (no CID).');
    return { story_cid: null, story_hash };
  }

  // Kubo RPC: POST /api/v0/add (multipart). Uses global fetch (Node 18+/20).
  const form = new FormData();
  form.append('file', new Blob([Buffer.from(storyText, 'utf8')]), 'story.txt');

  const headers = {};
  if (process.env.IPFS_API_AUTH) headers['Authorization'] = process.env.IPFS_API_AUTH;

  const res = await fetch(`${apiUrl.replace(/\/$/, '')}/api/v0/add?pin=true&cid-version=1`, {
    method: 'POST', body: form, headers,
  });
  if (!res.ok) throw new Error(`IPFS add failed: ${res.status} ${await res.text()}`);
  const out = await res.json(); // { Name, Hash, Size }
  return { story_cid: out.Hash, story_hash };
}

module.exports = { pinStory, sha256HexUtf8 };
