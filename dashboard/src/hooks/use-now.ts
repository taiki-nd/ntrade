"use client";

import * as React from "react";

/** 現在時刻（ミリ秒）。保有時間・注文の残り時間の表示用に `intervalMs` ごとに更新する */
export function useNow(intervalMs = 30000): number {
  const [now, setNow] = React.useState(() => Date.now());
  React.useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);
  return now;
}
