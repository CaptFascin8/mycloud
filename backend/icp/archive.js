'use strict';
// ============================================================================
//  archive.js — the daily H&G -> canister push.
//
//  Selects ceremonies that are (a) terminally settled and (b) past the 30-day
//  Blessed-Board window and (c) not yet on chain, builds the record with the
//  REAL engine assembler, pins the story to IPFS if consented, converts to the
//  canister SettlementRecordInput, and calls archive_ceremony. Idempotent:
//  AlreadyArchived is treated as success; each record is pushed at most once.
//
//  Per-record error isolation: one bad record never aborts the batch.
// ============================================================================

const engine = require('../engine');               // buildSettlementRecord (no side effects on require)
const { toSettlementRecordInput } = require('./convert');
const { pinStory } = require('./pinStory');

const WINDOW_DAYS   = parseInt(process.env.ICP_ARCHIVE_WINDOW_DAYS || '30', 10);
const MAX_ATTEMPTS  = parseInt(process.env.ICP_ARCHIVE_MAX_ATTEMPTS || '5', 10);
const BATCH_LIMIT   = parseInt(process.env.ICP_ARCHIVE_BATCH || '25', 10);

/** Render a HopeAndGraceError variant into a readable string. */
function describeErr(err) {
  if (!err || typeof err !== 'object') return String(err);
  const [k, v] = Object.entries(err)[0] || ['Unknown', null];
  return v === null ? k : `${k}: ${typeof v === 'bigint' ? v.toString() : v}`;
}

/** Best-effort: fetch the soul's story + consent for a blessing. Never throws. */
async function getStoryForArchive(db, blessing) {
  try {
    const [[soul]] = await db.query(
      'SELECT story_text, need_description, share_story FROM souls WHERE id = ?',
      [blessing.soul_id]
    );
    if (!soul) return null;
    // Consent: explicit share_story flag if present; default to NOT sharing if unknown.
    const consented = soul.share_story === 1 || soul.share_story === true;
    const text = (soul.story_text || soul.need_description || '').trim();
    return consented && text ? text : null;
  } catch (e) {
    // share_story column may not exist yet — fail closed (no story), don't crash.
    return null;
  }
}

async function markArchived(db, blessingId, ref, story) {
  await db.query(
    `UPDATE blessings_history
       SET archived_on_chain = TRUE,
           canister_ceremony_number = ?,
           content_hash = ?,
           archived_at_ns = ?,
           story_cid = ?,
           story_hash = ?,
           archived_at = NOW(),
           archive_last_error = NULL
     WHERE id = ?`,
    [
      ref?.ceremony_number != null ? ref.ceremony_number.toString() : null,
      ref?.content_hash ?? null,
      ref?.archived_at_ns != null ? ref.archived_at_ns.toString() : null,
      story?.story_cid ?? null,
      story?.story_hash ?? null,
      blessingId,
    ]
  );
}

async function recordFailure(db, blessingId, message) {
  await db.query(
    `UPDATE blessings_history
       SET archive_attempts = archive_attempts + 1, archive_last_error = ?
     WHERE id = ?`,
    [String(message).slice(0, 1000), blessingId]
  );
}

/**
 * Run the archive sweep.
 * @param {object} db  the mysql2 promise pool (require('./db/pool'))
 * @param {object} [opts] { dryRun?: boolean }  dryRun builds + converts but does not call the canister
 */
async function archiveSettledCeremonies(db, opts = {}) {
  const dryRun = !!opts.dryRun;
  const summary = { selected: 0, archived: 0, alreadyArchived: 0, failed: 0, skipped: 0, dryRun };

  const [candidates] = await db.query(
    `SELECT id, ceremony_number, soul_id
       FROM blessings_history
      WHERE archived_on_chain = FALSE
        AND archive_attempts < ?
        AND ( soul_reverted_at IS NOT NULL
              OR (soul_claimed = TRUE AND created_at <= DATE_SUB(NOW(), INTERVAL ? DAY)) )
      ORDER BY ceremony_number ASC
      LIMIT ?`,
    [MAX_ATTEMPTS, WINDOW_DAYS, BATCH_LIMIT]
  );
  summary.selected = candidates.length;
  if (candidates.length === 0) {
    console.log('[ICP Archive] Nothing due.');
    return summary;
  }

  // Lazily build the actor only when we actually have work and aren't dry-running.
  let actor = null, ctx = null;
  if (!dryRun) {
    ({ actor, ...ctx } = await require('./actor').getArchiveActor());
    console.log(`[ICP Archive] network=${ctx.network} canister=${ctx.canisterId} writer=${ctx.principal}`);
  }

  for (const c of candidates) {
    try {
      const rec = await engine.buildSettlementRecord(db, c.id);
      if (!rec || rec.outcome === 'pending') { summary.skipped++; continue; }

      // Pin the story only if the soul consented and we have text.
      let story = {};
      const storyText = await getStoryForArchive(db, c);
      if (storyText) {
        try { story = await pinStory(storyText); }
        catch (e) { console.warn(`[ICP Archive] story pin failed for #${rec.ceremony_number}: ${e.message} (archiving without story)`); }
      }

      const input = toSettlementRecordInput(rec, story);

      if (dryRun) {
        console.log(`[ICP Archive][dry] #${rec.ceremony_number} ok — pool_total_cents=${input.pool_total_cents} outcome=${Object.keys(input.outcome)[0]} ledger=${input.ledger.length}`);
        summary.archived++;
        continue;
      }

      const result = await actor.archive_ceremony(input);
      if ('Ok' in result) {
        await markArchived(db, c.id, result.Ok, story);
        summary.archived++;
        console.log(`[ICP Archive] #${rec.ceremony_number} archived — hash=${result.Ok.content_hash}`);
      } else if (result.Err && 'AlreadyArchived' in result.Err) {
        // Idempotent: the canister already has it. Mark locally so we stop retrying.
        await markArchived(db, c.id, { ceremony_number: result.Err.AlreadyArchived }, story);
        summary.alreadyArchived++;
        console.log(`[ICP Archive] #${rec.ceremony_number} already on chain — reconciled.`);
      } else {
        const msg = describeErr(result.Err);
        await recordFailure(db, c.id, msg);
        summary.failed++;
        console.error(`[ICP Archive] #${rec.ceremony_number} REJECTED — ${msg}`);
      }
    } catch (e) {
      await recordFailure(db, c.id, e.message).catch(() => {});
      summary.failed++;
      console.error(`[ICP Archive] ceremony #${c.ceremony_number} error: ${e.message}`);
    }
  }

  console.log(`[ICP Archive] done — ${JSON.stringify(summary)}`);
  return summary;
}

module.exports = { archiveSettledCeremonies };
