'use strict';
// ============================================================================
//  identity.js — load the H&G *writer* service identity.
//
//  This is the LOW-PRIVILEGE operational key whose only power is calling
//  archive_ceremony / put_legal_doc. It is NOT the canister owner/controller.
//  Its principal must be authorized on the canister via add_writer().
//
//  Generate one (off the shared box ideally), e.g. with dfx:
//      dfx identity new hg-writer --storage-mode=password-protected
//      dfx identity export hg-writer > hg-writer.pem   # keep this file 0600, OUT of git
//  Then print its principal to hand to mycloud Claude for add_writer():
//      dfx identity get-principal --identity hg-writer
//
//  Point ICP_WRITER_PEM at the .pem path in .env.
// ============================================================================

const fs = require('fs');

/**
 * Load a Secp256k1 or Ed25519 identity from a PEM file.
 * Lazily requires @dfinity packages so the rest of the app doesn't hard-depend
 * on them being installed until the archive job is actually wired on.
 */
function loadWriterIdentity() {
  const pemPath = process.env.ICP_WRITER_PEM;
  if (!pemPath) throw new Error('ICP_WRITER_PEM not set (path to the writer identity .pem)');
  const pem = fs.readFileSync(pemPath, 'utf8');

  // EC PRIVATE KEY -> secp256k1; PRIVATE KEY -> ed25519 (typical dfx exports).
  if (pem.includes('EC PRIVATE KEY')) {
    const { Secp256k1KeyIdentity } = require('@dfinity/identity-secp256k1');
    return Secp256k1KeyIdentity.fromPem(pem);
  }
  const { Ed25519KeyIdentity } = require('@dfinity/identity');
  // Ed25519KeyIdentity has no direct fromPem in all versions; support raw seed too.
  if (typeof Ed25519KeyIdentity.fromPem === 'function') {
    return Ed25519KeyIdentity.fromPem(pem);
  }
  throw new Error('Unsupported PEM format. Use a secp256k1 (EC PRIVATE KEY) identity, ' +
    'or install a @dfinity/identity version that supports Ed25519 fromPem.');
}

module.exports = { loadWriterIdentity };
