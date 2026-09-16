"use client";

import * as React from "react";

function subscribe(callback: () => void) {
  window.addEventListener("hashchange", callback);
  return () => window.removeEventListener("hashchange", callback);
}

function getSnapshot() {
  return window.location.hash.replace(/^#/, "");
}

function getServerSnapshot() {
  return "";
}

/** URL のハッシュ（`#` 抜き）を購読する。サイドバーとタブの同期に使う。 */
export function useHash(): string {
  return React.useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}

export function setHash(value: string) {
  if (typeof window === "undefined") return;
  const next = value ? `#${value}` : "";
  if (window.location.hash !== next) {
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}${next}`);
    window.dispatchEvent(new HashChangeEvent("hashchange"));
  }
}
