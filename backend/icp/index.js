'use strict';
// ============================================================================
//  index.js — entry points for the ICP archive module.
//
//  CLI (run by hand or by an external scheduler):
//      node backend/icp/index.js --dry       # build+convert only, no canister calls
//      node backend/icp/index.js --once      # one real sweep, then exit
//
//  In-app cron: from engine.js scheduleTasks(), add alongside the other crons:
//      const { registerArchiveCron } = require('./icp');
//      registerArchiveCron(cron, db);        // daily 08:00 UTC by default
//  (08:00 UTC sits just after the 07:00 soul-settlement sweep, so anything that
//   reverts today is eligible to archive in the same morning.)
// ============================================================================

const { archiveSettledCeremonies } = require('./archive');

/** Register the daily archive cron. `cron` is the node-cron module. */
function registerArchiveCron(cron, db, schedule = process.env.ICP_ARCHIVE_CRON || '0 8 * * *') {
  if (process.env.ICP_ARCHIVE_ENABLED !== 'true') {
    console.log('[ICP Archive] cron not registered (set ICP_ARCHIVE_ENABLED=true to enable).');
    return null;
  }
  console.log(`[ICP Archive] cron registered @ "${schedule}"`);
  return cron.schedule(schedule, async () => {
    try { await archiveSettledCeremonies(db); }
    catch (e) { console.error('[ICP Archive] cron run failed:', e.message); }
  });
}

module.exports = { archiveSettledCeremonies, registerArchiveCron };

// ---- CLI ----
if (require.main === module) {
  require('dotenv').config();
  const db = require('../db/pool');
  const dryRun = process.argv.includes('--dry');
  if (!dryRun && !process.argv.includes('--once')) {
    console.log('Usage: node backend/icp/index.js [--dry | --once]');
    process.exit(1);
  }
  archiveSettledCeremonies(db, { dryRun })
    .then((s) => { console.log('Result:', s); return db.end?.(); })
    .then(() => process.exit(0))
    .catch((e) => { console.error(e); process.exit(1); });
}
