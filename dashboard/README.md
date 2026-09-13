# Next.js + shadcn/ui Scaffold (AI-First Frontend Starter)

AIエージェントを活用したフロントエンド開発において、**「トークンの浪費」** と **「AI特有の安っぽいデザイン（AI感）」** を徹底的に防ぐために設計されたNext.jsスターターテンプレートです。

---

## 🌟 特徴

1. **頻出コンポーネントの事前配備（shadcn/ui）**
   - ボタン、カード、ダイアログ、テーブル、フォーム部品、トーストなど **20以上の主要パーツ** が最初から `@/components/ui/` に配置済み。
   - AIにパーツを1からCSSで書かせないため、**出力トークンを最大70%削減**。
2. **「AI感」を消すデザインシステム**
   - 落ち着いたモノトーン・セマンティックカラー（OKLCHベース）。
   - ダークモード標準対応（`next-themes`）。
   - 安っぽいグラデーションや原色の乱用を排除。
3. **洗練されたレイアウトテンプレート**
   - `DashboardLayout`: サイドバー・ヘッダー・レスポンシブドロワー完備の管理画面枠
   - `MarketingLayout`: トップナビ・フッター付きのLP/サービスサイト枠
   - `AuthLayout`: 中央カード配置のログイン・会員登録枠
4. **AI自動制約ルール（`AGENTS.md`, `.antigravity/rules/`）同梱**
   - AIがこのプロジェクトを読み込んだ際、自動的に「既存パーツを使う」「AI手癖デザイン禁止」のルールが適用されます。

---

## 🚀 新しいプロジェクトでの使い回し方（複製手順）

今後新しいアプリを作るときは、本スキャフォールドをコピーして利用します。

```bash
# 1. テンプレートを新規プロジェクト名でコピー
cp -r /Users/nodataiki/.gemini/antigravity-cli/scratch/next-scaffold ./my-new-app

# 2. 移動して開発開始
cd my-new-app
npm run dev
```

---

## 💡 AIへのプロンプト指示のコツ

AIに画面を作らせる際は、以下のように**「レイアウト」と「使うUIパーツ」を指定するだけ**で、最短のトークンで最高精度のコードが出力されます。

```markdown
「@/components/layouts/dashboard-layout.tsx を使い、
ユーザー一覧画面を実装してください。
UIパーツはすべて @/components/ui/ (Table, Badge, Button, Input, Dialog) を利用し、
独自CSSや余分なスタイルは書かないでください。」
```

---

## 📦 同梱されている主要UIコンポーネント

* **アクション**: `Button`
* **コンテナ**: `Card`, `Accordion`, `Separator`
* **入力・フォーム**: `Input`, `Label`, `Textarea`, `Select`, `Checkbox`, `Switch`
* **オーバーレイ**: `Dialog`, `Sheet`, `DropdownMenu`, `Tooltip`, `Popover`
* **データ表示**: `Table`, `Badge`, `Avatar`, `Tabs`, `Skeleton`
* **フィードバック**: `Sonner` (Toast)
* **ナビゲーション**: `Breadcrumb`
* **アイコン**: `lucide-react`
* **テーマ**: `ThemeToggle`, `ThemeProvider` (`next-themes`)

---

## 🛠️ コマンド

```bash
npm run dev      # 開発サーバー起動 (http://localhost:3000)
npm run build    # 本番ビルド
npm run lint     # リントチェック
```
