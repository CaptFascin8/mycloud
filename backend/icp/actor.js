'use strict';
// ============================================================================
//  actor.js — build the @dfinity/agent Actor for the hopeandgrace canister.
//
//  Network is chosen by env so we can test against a LOCAL replica first
//  (per the spec's "local through Phase 1, switch canister ID for mainnet"):
//
//    ICP_NETWORK = 'local' | 'ic'        (default 'local')
//    ICP_HOST                            (default http://127.0.0.1:4943 local, https://icp-api.io ic)
//    ICP_CANISTER_ID                     (required — from mycloud deploy)
//    ICP_WRITER_PEM                      (path to writer identity pem)
//
//  IMPORTANT: on 'local' we fetchRootKey(); NEVER do that on mainnet (it would
//  disable response verification). The guard below enforces that.
// ============================================================================

const { loadWriterIdentity } = require('./identity');
// Stand-in factory for local dev. Swap to the GENERATED declarations after deploy.
const { idlFactory } = require('./idl');

let _cached = null;

async function getArchiveActor() {
  if (_cached) return _cached;

  const network = process.env.ICP_NETWORK || 'local';
  const isMainnet = network === 'ic';
  const host = process.env.ICP_HOST || (isMainnet ? 'https://icp-api.io' : 'http://127.0.0.1:4943');
  const canisterId = process.env.ICP_CANISTER_ID;
  if (!canisterId) throw new Error('ICP_CANISTER_ID not set (the deployed hopeandgrace canister id)');

  const { HttpAgent, Actor } = require('@dfinity/agent');
  const identity = loadWriterIdentity();

  const agent = await HttpAgent.create({ host, identity });

  if (!isMainnet) {
    // Local replica only — fetch the root key so the agent trusts the dev replica.
    await agent.fetchRootKey();
  } else if (host.includes('127.0.0.1') || host.includes('localhost')) {
    throw new Error('Refusing to run on mainnet with a localhost host — check ICP_HOST/ICP_NETWORK.');
  }

  const actor = Actor.createActor(idlFactory, { agent, canisterId });
  _cached = { actor, network, host, canisterId, principal: identity.getPrincipal().toText() };
  return _cached;
}

module.exports = { getArchiveActor };
