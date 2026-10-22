# 08. 日中短期取引 × LLM（予約注文ベース）

> 2026-10-13 に判断ロジックを全面的に置き換えた。ファンダメンタルズ判断（07、削除済み）と条件付きプラン（01・02・06 の該当箇所）は廃止。
> 01〜06 のうち、ここと食い違う記述はこの文書を正とする。

## 1. 方針

| 項目 | 内容 |
| :--- | :--- |
| 保有時間 | 15 分〜2 時間程度（日中で決着させる） |
| 判断材料 | 価格の数値だけ（1H/15M/5M の生 OHLC、スイング高安、前日・当日・アジア時間の高安、キリ番、ATR、セッション）。ファンダメンタルズ・Web 情報・チャート画像は使わない |
| 新規エントリー | **予約注文（指値 LIMIT・逆指値 STOP）だけ**。成行は出さない |
| 注文の期限 | 必須。LLM が注文ごとに決め、`plan_max_hours`（既定 8 時間）を上限とする |
| 既存の注文・建玉 | 判断のたびに LLM が見直す（取消・変更・決済・SL/TP 変更） |
| LLM | `claude -p` サブプロセス。モデルは opus（`NTRADE_LLM_MODEL` で変更可）、タイムアウト 180 秒 |
| 対象銘柄の初期値 | USDJPY / EURUSD / XAUUSD（口座のサフィックスを補う）。設定画面で任意の銘柄を追加・削除できる |

### なぜ予約注文か

`claude -p` の判断には足の確定から 25〜60 秒かかる。成行では判断した価格から離れて約定し、SL/TP までの距離も崩れる。
価格を決めてブローカーに置いておけば、遅れは「注文を置くまでの時間」になるだけで、約定はブローカーが価格どおりに行う。
判断の合間（判断間隔が 5 分より長い場合も）でも約定を取りこぼさない。

## 2. 判断サイクル

5 分足の確定ごと（`barDelaySecs` 後）に全銘柄を並行に回す。LLM を呼ぶのは判断間隔（`decisionIntervalMinutes`）の足だけ。

1. Snapshot を作る（口座の建玉・**予約注文**と直近 3 件の判断を含む。現在値は気配値の仲値）
2. 終値判定の損切り（`stop_mode = close` の建玉のみ。5 分足ごと）
3. LLM 判断
4. `manage` の操作を先に実行する（取り消した注文の枠を新規注文に使えるように）
5. 新規の予約注文を事後ガードにかけ、通れば発注する
6. 判断ログ（CoT）を保存する。ガード不通過・発注失敗・操作の結果は `guard_result` に入れる

### LLM の出力（`TradeDecision`）

| フィールド | 意味 |
| :--- | :--- |
| `action` | 新しい予約注文を置くなら BUY / SELL、置かないなら HOLD |
| `entry_type` | LIMIT / STOP |
| `entry_price` / `stop_loss` / `take_profit` | 注文の価格と SL/TP（絶対価格） |
| `expiry_minutes` | 注文の有効期限（今から何分後か） |
| `analysis` | `market_structure`（流れ）/ `price_levels`（価格の根拠）/ `invalidation`（SL の根拠）/ `conflicts`（反対材料） |
| `manage[]` | `{target: ORDER\|POSITION, id, op: CANCEL\|CLOSE\|MODIFY, entry_price?, stop_loss?, take_profit?, expiry_minutes?, reason}`。挙げなかった注文・建玉はそのまま |

判断ログの列は、`macro_context` に流れの読み、`order_flow` に価格の根拠を入れる。

## 3. 事後ガード

新規注文にだけ掛ける。`manage` は「ブローカーで実行できる形か」だけを見て、操作の是非は LLM の裁量に任せる。

| ガード | 内容 |
| :--- | :--- |
| `CONFIDENCE_LOW` / `RR_TOO_LOW` / `SL_*` / `SPREAD_TOO_WIDE` / `DAILY_LOSS_LIMIT` / `NEWS_BLACKOUT` | 従来どおり（RR・SL 幅は注文の価格から測る） |
| `ENTRY_TYPE` | LIMIT か STOP であること |
| `ORDER_PRICE_SIDE` | 指値は現在値より有利な側（BUY は下・SELL は上）、逆指値は不利な側（BUY は上・SELL は下） |
| `ORDER_EXPIRY` | 期限が 1 分〜`plan_max_hours` |
| `MAX_POSITIONS` | **建玉 + 約定待ちの予約注文**の数で上限を見る |
| `MANAGE_INVALID`（操作） | MODIFY の SL/TP が向きに対して逆側でないか（注文は新しい価格、建玉は現在値が基準） |

## 4. 執行と照合

- 発注: `ProtoOANewOrderReq`（LIMIT/STOP、GOOD_TILL_DATE、絶対価格の SL/TP、`comment` = 判断 ID、`label` = ntrade）。価格は銘柄の桁数に丸める
- `stop_mode = close` のときは、注文に置く SL をハードSL（LLM の SL から `hard_stop_atr` × 5M ATR 外側）にし、LLM の SL を終値判定ラインとして注文に記録しておく。約定した建玉がそれを引き継ぐ
- ctrader-rs の request/response では注文の応答を拾えないため、clientMsgId 無しで送り、非請求の執行イベント（受理 = ORDER_ACCEPTED、変更 = ORDER_REPLACED、取消 = ORDER_CANCELLED）を `clientOrderId` / 注文 ID で待つ
- 予約注文は SQLite の `working_orders` に保存する（再起動しても判断との対応と終値判定ラインを失わない）
- 照合（60 秒ごと + 判断サイクル後）は 1 回の reconcile で建玉と注文を取り、
  - ブローカーから消えた注文は `order_details` で結末を引く。約定なら建玉 ID を得て、判断 ID・終値判定ライン・損切りの根拠を建玉へ引き継ぐ
  - 照合までの間に約定して決済まで済んだ建玉は、約定履歴から決済として記録する
  - ブローカーにだけある注文・建玉は取り込み、`comment` が判断 ID なら紐付ける
- LLM の `CLOSE` による決済は決済理由 `DISCRETIONARY` で記録する
- 緊急停止は予約注文をすべて取り消してから建玉を決済する

## 5. リプレイ

- Snapshot は本番と同じ（`snapshot-v3`）。ファンダメンタルズを使わなくなったので、本番と同じ入力で評価できる
- 採点: 予約注文が期限内に 5 分足で届いたら約定（指値は指値の価格、逆指値は窓を空けたら始値）→ 約定後に SL/TP のどちらが先かを見る。約定した足では SL 側だけを見る（TP 側は約定前の値動きかもしれないので不利側に倒す）。届かなければ `ORDER_EXPIRED`
- 口座状態を持たないので `manage` は採点しない
- 旧方式の run（成行・条件付きプラン）も読める。`rescore` では成行は従来どおり、プラン成立後の再判断は採点し直さない

## 6. 廃止したもの

- `src/fundamentals/`、`config/fundamentals.toml`、`/api/fundamentals*`、設定の `eventDecisions`（指標発表後の判断）
- 条件付きプラン（`PlanBook`・`/api/plans`・成立時の再判断・リプレイの `--mechanical-plans`）
- 新規の成行発注（PoC の `poc_ctrader` は検証用に残す）
