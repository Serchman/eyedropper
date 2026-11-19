// Ordering of "state" payloads from Rust.
//
// Rust takes each snapshot under its lock but sends it after releasing the
// lock, so a once-a-second tick can arrive after the broadcast of a newer
// change (e.g. a removed card would come back for a second). Every payload
// carries `rev`, which Rust raises on each change to the patient list, and
// Rust's `now`. A window draws a payload only if it is not older than the
// one it last drew.

/**
 * True if `next` is older than `prev` (the payload last drawn) and must be
 * ignored: a lower `rev`, or the same `rev` with an older `now` (e.g. a late
 * get_state reply). The same `rev` with a newer `now` is a plain tick.
 */
export function isOutdated(prev, next) {
  if (!prev) return false;
  if (next.rev !== prev.rev) return next.rev < prev.rev;
  return next.now < prev.now;
}
