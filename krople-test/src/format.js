// Formats a time difference in milliseconds (end time minus now).
//   time left (positive)          -> "0:18", "1:45"
//   run out (zero or negative)    -> "+0:00", "+0:12" (overdue, counting up)
export function formatTime(diffMs) {
  const overdue = diffMs <= 0;
  // Round time left up and overdue time down, so the display goes
  // 0:02, 0:01, +0:00, +0:01 without showing "0:00".
  const totalSec = overdue ? Math.floor(-diffMs / 1000) : Math.ceil(diffMs / 1000);
  const m = Math.floor(totalSec / 60);
  const s = String(totalSec % 60).padStart(2, "0");
  return (overdue ? "+" : "") + `${m}:${s}`;
}

// Formats how long a phase has been over (now minus end time) as whole
// minutes, counting up from "+0": 0..59 s -> "+0", 60 s -> "+1".
// Returned in two parts so the card can draw "min" smaller.
export function formatOverdue(overdueMs) {
  const minutes = Math.floor(Math.max(overdueMs, 0) / 60000);
  return { value: "+" + minutes, unit: "min" };
}
