# AI Agent Instructions for Frontend Development

本リポジトリは、**「AIのトークン浪費を防ぐ」** および **「AI特有の安っぽいデザイン（AI感）を完全に排除する」** ために高度に統制されたNext.js + shadcn/ui スキャフォールドです。

AIエージェント（Antigravity, Cursor, Claude Code等）は、本プロジェクトでフロントエンドコードを生成・編集する際、以下の【絶対厳守ルール】に従ってください。

---

## 1. コンポーネントのゼロスクラッチ絶対禁止（トークン節約）

* **UIパーツの自作禁止**: ボタン、入力欄、カード、モーダル、ドロップダウン、テーブル等のUIパーツを生のHTML (`<button>`, `<input>`) や生のTailwindクラスでゼロから作成してはいけません。
* **必ず既存コンポーネントをインポートすること**:
  * すべてのUI要素は `@/components/ui/` 配下の既存パーツ（例: `Button`, `Card`, `Input`, `Dialog`, `Table`, `Badge`, `Sheet` 等）を呼び出してください。
* **画面レイアウトの再利用**:
  * ダッシュボード画面: `@/components/layouts/dashboard-layout` を使用
  * LP・公開Webサイト: `@/components/layouts/marketing-layout` を使用
  * ログイン・認証画面: `@/components/layouts/auth-layout` を使用

---

## 2. 「AI臭さ」の徹底排除ルール（デザイン品質の担保）

AIが生成しがちな以下の安っぽいデザイン手癖を**固く禁止**します：

1. **グラデーションの禁止**:
   * `bg-gradient-to-r from-purple-500 to-indigo-600` のような派手なグラデーションは原則使用禁止。背景は洗練された `bg-background` または `bg-muted/30` のフラットなミニマルデザインを維持すること。
2. **生カラー（ハードコード色）の禁止**:
   * `text-blue-500`, `bg-gray-100` などの色直接指定は禁止。
   * 必ずセマンティックカラー（`bg-background`, `text-foreground`, `text-muted-foreground`, `border-border`, `bg-accent` など）を使用すること（ダークモード自動対応のため）。
3. **過剰な装飾・アニメーションの禁止**:
   * 意味のないドロップシャドウ（`shadow-2xl`）やパルスアニメーション（`animate-pulse`）を勝手につけないこと。
4. **アイコンの統一**:
   * アイコンはすべて `lucide-react` からインポートすること。

---

## 3. ユーザーからのカラー・トーン指定時の絶対プロトコル

ユーザーから「青系にして」「Linear風のダークにして」「Notion風の温かみあるトーンで」「アクセントはエメラルドグリーン」などのカラーや雰囲気の指定があった場合、AIは以下のプロトコルを**必ず厳守**してください：

1. **コンポーネント側への色直接指定は固く禁止（厳罰対象）**:
   * `<button className="bg-blue-600 ...">` や `<div className="text-slate-800 ...">` のように、UIコンポーネント側に色を直接書き込んではいけません。
   * トークン浪費、ダークモードの破綻、デザイン崩壊の原因となります。
   * UIコンポーネント側は常にセマンティックトークン（`bg-primary`, `bg-background`, `border-border`, `text-muted-foreground` 等）のみで記述してください。

2. **色・質感の変更は CSS 変数（`globals.css`）または data 属性で完結させること**:
   * ユーザーから指定された色やトーンは、UIのTailwindクラスを書き換えるのではなく、`src/app/globals.css` のセマンティック変数を調整するか、`html` タグに以下のプリセット属性（2大直交軸）を付与することで制御してください：
     * **アクセントカラー軸**: `data-accent-color="blue|emerald|violet|rose|orange"`（ボタン、バッジ、リング、ホバーのティント背景色 `--accent` / `--accent-foreground` も全自動連動）
     * **ベーストーン軸**: `data-base-tone="slate|stone|oled"`（背景、カード、枠線のトーン）

3. **AI標準カラー＆トーンレシピ辞書（ユーザー指定時の推奨設定）**:
   ユーザーの要望に応じて、以下の調和されたOKLCHカラーレシピを使用すること：

   * **アクセントカラー（主要アクション・ボタン・リング `--primary`, `--ring`, `--accent`）**:
     * **Blue (Stripe風・信頼・SaaS標準)**: `data-accent-color="blue"`
       * Light: `--primary: oklch(0.55 0.22 255); --accent: oklch(0.95 0.035 255);` / Dark: `--primary: oklch(0.65 0.20 255); --accent: oklch(0.24 0.05 255);`
     * **Emerald (Supabase風・Fintech・自然・開発者)**: `data-accent-color="emerald"`
       * Light: `--primary: oklch(0.58 0.19 155); --accent: oklch(0.95 0.03 155);` / Dark: `--primary: oklch(0.68 0.18 155); --accent: oklch(0.24 0.045 155);`
     * **Violet (Linear風・AI・近未来ツール)**: `data-accent-color="violet"`
       * Light: `--primary: oklch(0.55 0.25 285); --accent: oklch(0.95 0.04 285);` / Dark: `--primary: oklch(0.66 0.23 285); --accent: oklch(0.25 0.055 285);`
     * **Rose (モダン・親しみ・B2C)**: `data-accent-color="rose"`
       * Light: `--primary: oklch(0.58 0.23 15); --accent: oklch(0.95 0.035 15);` / Dark: `--primary: oklch(0.68 0.20 15); --accent: oklch(0.24 0.05 15);`
     * **Orange (活力・クリエイティブ)**: `data-accent-color="orange"`
       * Light: `--primary: oklch(0.62 0.20 48); --accent: oklch(0.95 0.035 48);` / Dark: `--primary: oklch(0.70 0.19 48); --accent: oklch(0.25 0.045 48);`

   * **ベーストーン（背景・カードの質感 `--background`, `--card`, `--muted`）**:
     * **Zinc (デフォルト)**: 純白・完全な無彩色モノトーン（Apple / Vercel風）
     * **Slate (クールグレー・青み系)**: 清潔感ある青みがかったグレー（GitHub / SaaS風）→ `data-base-tone="slate"`
     * **Stone (ウォームグレー・ベージュ系)**: 優しい生成り・和紙・チャコール（Notion / 落ち着いたメディア風）→ `data-base-tone="stone"`
     * **OLED (漆黒系・ハイコントラスト)**: 完全な黒（`#000000`）のダークモード → `data-base-tone="oled"`

---

## 4. コード生成の最小化

* コードを出力する際は、ページ全体の再出力ではなく変更対象のコンポーネント・関数のみを局所的に記述すること。
* コンポーネントの props は型安全に TypeScript で定義すること。

<!-- BEGIN:nextjs-agent-rules -->

# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
