'use strict';
// ============================================================================
//  migrate_icp.js — add on-chain archive bookkeeping columns to
//  blessings_history. Idempotent: checks information_schema before each ADD,
//  so it is safe to run repeatedly (matches the project's migration discipline).
//
//  Run from backend/:  node migrate_icp.js
// ============================================================================

require('dotenv').config();
const db = require('./db/pool');

const COLUMNS = [
  ['archived_on_chain',          'BOOLEAN NOT NULL DEFAULT FALSE'],
  ['canister_ceremony_number',   'BIGINT UNSIGNED NULL'],
  ['content_hash',               'VARCHAR(64) NULL'],
  ['archived_at_ns',             'BIGINT UNSIGNED NULL'],
  ['story_cid',                  'VARCHAR(255) NULL'],
  ['story_hash',                 'VARCHAR(64) NULL'],
  ['archive_attempts',           'INT NOT NULL DEFAULT 0'],
  ['archive_last_error',         'TEXT NULL'],
  ['archived_at',                'DATETIME NULL'],
];

async function columnExists(table, column) {
  const [[row]] = await db.query(
    `SELECT COUNT(*) AS n FROM information_schema.COLUMNS
      WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND COLUMN_NAME = ?`,
    [table, column]
  );
  return row.n > 0;
}

async function indexExists(table, indexName) {
  const [[row]] = await db.query(
    `SELECT COUNT(*) AS n FROM information_schema.STATISTICS
      WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND INDEX_NAME = ?`,
    [table, indexName]
  );
  return row.n > 0;
}

(async () => {
  const table = 'blessings_history';
  try {
    for (const [name, def] of COLUMNS) {
      if (await columnExists(table, name)) {
        console.log(`= ${table}.${name} already exists`);
      } else {
        await db.query(`ALTER TABLE ${table} ADD COLUMN ${name} ${def}`);
        console.log(`+ added ${table}.${name}`);
      }
    }
    // Index to make the daily "what's due" selection cheap.
    if (await indexExists(table, 'idx_archive_due')) {
      console.log('= idx_archive_due already exists');
    } else {
      await db.query(`ALTER TABLE ${table} ADD INDEX idx_archive_due (archived_on_chain, archive_attempts)`);
      console.log('+ added index idx_archive_due');
    }
    console.log('\nMigration complete.');
    process.exit(0);
  } catch (e) {
    console.error('Migration failed:', e.message);
    process.exit(1);
  }
})();
