/** エンジンの時刻（"YYYY-MM-DD HH:MM:SS"、UTC）をミリ秒にする。読めなければ null */
export function parseEngineTime(s: string | undefined | null): number | null {
  if (!s) return null;
  const ms = Date.parse(`${s.trim().replace(" UTC", "").replace(" ", "T")}Z`);
  return Number.isNaN(ms) ? null : ms;
}

/** 経過・残り時間を「1時間23分」「45分」の形にする（1 分未満は「1分未満」） */
export function formatDuration(ms: number): string {
  const min = Math.floor(Math.abs(ms) / 60000);
  if (min < 1) return "1分未満";
  const h = Math.floor(min / 60);
  const m = min % 60;
  return h > 0 ? `${h}時間${m > 0 ? `${m}分` : ""}` : `${m}分`;
}
