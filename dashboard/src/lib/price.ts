/**
 * 価格の表示桁数。エンジンの銘柄仕様が手元に無い一覧表示用の推測（engine の instrument::guess と同じ規則）。
 * FX の JPY クロスは 3 桁、その他の FX は 5 桁、XAU 2 桁、XAG 3 桁、原油 2 桁、株価指数 1 桁。
 */
export function priceDigits(symbol: string): number {
  const s = symbol.toUpperCase();
  if (s.includes("XAU") || s.includes("GOLD")) return 2;
  if (s.includes("XAG") || s.includes("SILVER")) return 3;
  if (["OIL", "WTI", "BRENT", "XTI", "XBR"].some((k) => s.includes(k))) return 2;
  const letters = s.match(/^[A-Z]+/)?.[0] ?? "";
  if (letters.length === 6) return s.includes("JPY") ? 3 : 5;
  if (/\d/.test(s)) return 1;
  return 5;
}

export function formatPrice(symbol: string, price: number): string {
  return price.toFixed(priceDigits(symbol));
}
