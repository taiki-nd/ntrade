# 金（XAUUSD）と株価指数 CFD（US500/US100/US30/GER40/JP225）の日中〜週次アノマリー：文献調査ノート

調査日：2026-10-22。前提は cTrader のゼロスプレッド＋手数料口座、Rust によるバー確定ベースの判断、SL 必須、SL 幅の上限は既定で H1 ATR×4。
注意：多くの数値は抄録・二次情報（WebSearch の要約）から取っており、全文で確認できたのは NY 連銀 Liberty Street の記事と QuantSeeker の記事だけ。数値はそのまま使わず、実装前に自前のデータで再検証すること。

---

## Q1. 株価指数：オーバーナイト／日中の分解と「オーバーナイト・ドリフト」

### Takeaway
米国株のリターンの大半は、歴史的には取引時間外（オーバーナイト）に生じてきた。特に欧州寄り付き前後の ET 2:00–3:00 の 1 時間に集中していたが、NY 連銀の著者ら自身が「2021–2025 年はほぼゼロに消えた」と報告している。出版後に消えたアノマリーの典型であり、そのまま実装する価値は低い。

### Cited Findings
- Boyarchenko, Larsen, Whelan「The Overnight Drift」RFS 36(9), 2023, pp.3502–3547（DOI 10.1093/rfs/hhad020）。2020 年の CEPR 版では「米国株プレミアムのほぼ 100% が ET 2:00–3:00 の 1 時間で得られている」と報告。メカニズムは、引け時点の売り越し（order imbalance）を吸収したディーラーへの在庫リスク補償 — [RFS](https://academic.oup.com/rfs/article-abstract/36/9/3502/7076616), [NY Fed SR917](https://www.newyorkfed.org/research/staff_reports/sr917), [CEPR DP14462](https://cepr.org/publications/dp14462)
- 出版後の検証（NY 連銀 Liberty Street、2026 年 7 月「The Disappearing Overnight Drift」）：E-mini S&P の 2:00–3:00 ET の窓は 1998–2020 年（5,691 日）で年率約 3.7%、ES の close-to-close 年率 5.9% の 60% 超を占めていた。2021年1月–2025年12月（1,245 日）では平均がほぼゼロで、NQ と YM でも同じく平坦 — [Liberty Street Economics](https://libertystreeteconomics.newyorkfed.org/2026/07/the-disappearing-overnight-drift/)
- 消えた原因は、引け前（15:15–16:15 ET）の signed volume の標準偏差が 6.5% から 2.9% に縮小したこと。VIX 平均は 20.4 から 19.4 とほぼ不変で、オーバーナイトの出来高シェアも 15% から 16% とほぼ不変。著者らは「引けの不均衡の分散が戻ればドリフトも戻る」と予想しているが、未検証 — [Liberty Street Economics](https://libertystreeteconomics.newyorkfed.org/2026/07/the-disappearing-overnight-drift/)
- NightShares のオーバーナイト ETF 2 本（2022 年 6 月上場）は約 14 か月で償還された — [Liberty Street Economics](https://libertystreeteconomics.newyorkfed.org/2026/07/the-disappearing-overnight-drift/)
- Lou, Polk, Skouras「A Tug of War: Overnight versus Intraday Expected Returns」（JFE 2019）は、株式の横断面アノマリー（モメンタムなど）をオーバーナイトと日中の成分に分解した研究。個別株の横断面が対象で、指数単体の売買ルールは直接は出てこない — [LSE PDF](https://personal.lse.ac.uk/polk/research/TugOfWar.pdf), [Yale 版](http://www.econ.yale.edu/~shiller/behfin/2015-04-11/lou_polk_skouras.pdf)
- 関連研究：Bogousslavsky（2021, JFE）「The cross-section of intraday and overnight returns」 — [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0304405X21000854)

### Inferences
- 「US500 を NY 引け（16:00 ET）で買い、翌 NY 寄り（9:30 ET）で売る」「2:00–3:00 ET だけ保有する」といったルールは、2021 年以降ほぼゼロの期待リターンになる。CFD でオーバーナイト保有すると金利（financing）がかかるので、期待値はマイナスになる可能性が高い。
- 例外は条件付きの版。引け前の大きな売り越し日（ES が引け前 1 時間で大きく下落した日）に限定すれば、メカニズムとしては残っている可能性がある。ただし 2021 年以降は不均衡の分散そのものが小さく、機会の頻度が少ない。
- 2:00–3:00 ET の 1 時間だけの保有なら日をまたがないので、financing は発生しない（ロールオーバー時刻をまたがないため）。それでも期待値がゼロ付近なので、手数料分だけ負ける。

### Gaps
- Cliff, Cooper, Gulen（2008）「Return Differences between Trading and Non-trading Hours: Like Night and Day」の具体的な数値は今回取得できず、出典 URL も未確認。
- GER40 と JP225 のオーバーナイト分解について、2018 年以降の論文は見つからなかった。

---

## Q2. 株価指数：日中モメンタム（前半 30 分 → 最後の 30 分）とガンマヘッジ

### Takeaway
Gao-Han-Li-Zhou（2018 JFE）の効果は 1993–2013 年の SPY で OOS R² 1.4–2.5% と小さく、有意ではあった。Baltussen ら（2021 JFE）は 60 超の先物で効果を確認し、ディーラーのショート・ガンマのヘッジに結びつけた。実務版にあたる Zarattini-Aziz-Barbon の「ノイズエリア」ブレイクアウトは、2007–2024 年で Sharpe 1.33 と報告されている。ただし 2025 年以降は「エッジが圧縮し、Sharpe はほぼゼロ」とする未査読のレプリケーションがある。米国指数の日中系では最有力の候補だが、直近の劣化には要注意。

### Cited Findings
- Gao, Han, Li, Zhou「Market intraday momentum」JFE 2018。SPY 1993–2013 年で、最初の 30 分（前日引け〜10:00 ET）のリターンが最後の 30 分（15:30–16:00）のリターンを予測する。OOS R² は前半 30 分のみで 1.4%（別版では 1.7%）、12 本目の 30 分足と組み合わせると 2.0%（別版では 2.53%）。効果はボラティリティの高い日、出来高の多い日、景気後退期、マクロ指標の発表日に強い。working paper 版は「妥当な取引コスト後も残る」としている — [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0304405X18301351), [SSRN WP PDF](https://www.smallake.kr/wp-content/uploads/2015/01/SSRN-id2440866.pdf), [著者 PDF](https://assets.super.so/e46b77e7-ee08-445e-b43f-4ffd88ae0a0e/files/ee7dac49-530b-4950-b5d0-e0b5eee08f2e.pdf)
- 国際比較：16 か国で OOS の予測力は多くの国で有意（Li ら, Reading/Lancaster WP） — [Reading accepted version](https://centaur.reading.ac.uk/95566/1/Accepted-Version.pdf), [FoFI 2020](http://wp.lancs.ac.uk/fofi2020/files/2020/04/FoFI-2020-092-Zeming-Li.pdf)。スウェーデン個別株の修士論文では頑健性がなく、OOS で過去平均モデルに負けた — [DiVA](https://www.diva-portal.org/smash/get/diva2:1878991/FULLTEXT01.pdf)
- S&P500 と CSI300 を対象とした研究は「パンデミック期には最後の 30 分の日中モメンタムは有意でない」と報告 — [ResearchGate](https://www.researchgate.net/publication/356901732_THE_IMPACT_OF_INTRADAY_MOMENTUM_ON_STOCK_RETURNS_EVIDENCE_FROM_SP500_AND_CSI300)
- Baltussen, Da, Lammers, Martens「Hedging demand and market intraday momentum」JFE 142(1), 2021, pp.377–403。1974–2020 年、株式・債券・商品・通貨の 60 超の先物で、「引け前 30 分以外の当日リターン」が最後の 30 分を正に予測する。予測された動きは翌日以降に反転する。メカニズムはオプションとレバレッジ ETF のマーケットメイカーによるガンマヘッジで、OptionMetrics から作ったネットガンマ（1996–2020 年）がマイナスの日ほど効果が強い — [SSRN](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=3760365), [著者 PDF](https://academicweb.nd.edu/~zda/intramom.pdf), [EconPapers](https://econpapers.repec.org/RePEc:eee:jfinec:v:142:y:2021:i:1:p:377-403)
- Zarattini, Aziz, Barbon「Beat the Market: An Effective Intraday Momentum Strategy for S&P500 ETF (SPY)」（2024）。2007–2024 年でコスト控除後の年率 19.6%、Sharpe 1.33、総リターン 1,985% と報告。ルールは次のとおり。時刻ごとに、過去 14 日間の「寄り付きからの絶対変動」の平均でバンド（Noise Area）を作り、価格がバンドを抜けた方向にエントリーする — [Concretum](https://concretumgroup.com/beat-the-market-an-effective-intraday-momentum-strategy-for-sp500-etf-spy/), [Univ. St. Gallen PDF](https://alexandria.unisg.ch/bitstreams/a99aba00-f967-49b3-aceb-f544dc386e0b/download), [replication GitHub](https://github.com/svettelnova/intraday-momentum-spy)
- レプリケーションと OOS 検証：
  - Paz（SSRN 7290621）は IQFeed の 1 分足で、2015–2024 年の in-sample を Sharpe 1.34 で再現した。OOS 期間は 2024年5月–2026年3月だが、OOS の数値は今回取得できなかった（403）— [SSRN](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=7290621)
  - codecat-ops の GitHub 再現（未査読）は 2007–2024 年で Sharpe 1.11 とし、「2025 年以降はエッジが圧縮し、SPY でも ES でも Sharpe はほぼ 0」と記載。ただし、検索要約で確認した後にリポジトリを開こうとすると 404 だった — [GitHub (404 at fetch time)](https://github.com/codecat-ops/zarattini-2024-momentum-spy)
  - Maróy（SSRN 5095349）はパラメータと出口ルールの最適化で改善できると主張 — [SSRN](https://papers.ssrn.com/sol3/Delivery.cfm/5095349.pdf?abstractid=5095349&mirid=1)

### Inferences（実装ルール案）
- ルール A（Gao 型）：US500 の M30 または M5 バー。シグナル r1 = 前日 16:00 ET の終値から当日 10:00 ET までのリターン。15:30 ET のバー確定時点で sign(r1) 方向に成行で入り、16:00 ET（CFD では 15:59 のバー確定）で手仕舞う。フィルターは「当日の実現ボラが高い日」「マクロ指標の発表日」。米国の夏時間・冬時間で UTC 時刻がずれるので、DST の処理が必須。
- ルール B（Baltussen 型）：シグナルは 9:30 から 15:30 ET までのリターン（前日終値〜15:30 でもよい）。15:30 に同じ方向へ入り、16:00 に手仕舞う。ネットガンマのデータは CFD 環境にないので、代わりに VIX が高い日や前日比の変動が大きい日に限定する、というのが実務上の近似になる。
- ルール C（Noise Area 型）：US500 または US100 の M30 判断。寄り付きからの乖離が過去 14 日の同時刻の平均絶対乖離を超えたら順張りで入る。ストップはバンドの内側に戻ったところかつ VWAP、16:00 ET には全決済。日をまたがないので financing はゼロ。
- コスト面：1 トレードあたりの期待値は 30 分保有で数 bp 程度と推定される。手数料（往復でおおむね 0.5–2 bp 相当か。ブローカー依存で未確認）を差し引くと薄い。2025 年以降の劣化報告もあるので、まず自前の 2018–2026 年データで年別の Sharpe を見るべき。
- SL ガード（H1 ATR×4）は 30 分保有なら問題にならない。

### Gaps
- 2018 年以降の米国指数を対象に Gao 型を査読付きで OOS 検証した研究は見つからなかった。
- Paz の OOS 数値と codecat の 2025 年以降の数値は、どちらも全文未確認。
- GER40 と JP225 での日中モメンタムは、16 か国研究には含まれる可能性が高いが、国別の数値は未確認。

---

## Q3. 株価指数：FOMC 前ドリフト

### Takeaway
Lucca-Moench（2015 JF）の FOMC 前 24 時間ドリフトは、Kurov-Wolfe-Gilbert（2021）によれば 2015 年以降はほぼ消えた。ただし記者会見のある会合や高 VIX 期には残っているという報告もあり、評価は定まっていない。年 8 回しか機会がないので、統計的な検証力も乏しい。

### Cited Findings
- Lucca-Moench：1994年9月–2011年3月、SPX の FOMC 前 24 時間の超過リターンは年率換算 3.89%、それ以外の全日は 0.88% — [NY Fed SR512](https://www.newyorkfed.org/medialibrary/media/research/staff_reports/sr512.pdf)
- Kurov, Wolfe, Gilbert（2021, Finance Research Letters 40）「The disappearing pre-FOMC announcement drift」。サンプルを 2019 年 12 月まで延長すると、ドリフトは 2015 年以降、記者会見の有無を問わずほぼ消えている — [EconPapers](https://econpapers.repec.org/RePEc:eee:finlet:v:40:y:2021:i:c:s1544612320315956), [ResearchGate](https://www.researchgate.net/publication/345940053_The_Disappearing_Pre-FOMC_Announcement_Drift)
- BIS WP 1079：2011 年以降、ドリフトは発表 2 日前に移り、規模も約 22bp に縮小 — [BIS](https://www.bis.org/publ/work1079.pdf)
- Cujean-Jaeger も「2015 年以降消えた」と記述 — [PDF](http://www.juliencujean.com/AJ6_02062023.pdf)
- 反論：2024 年の Applied Economics 論文は、記者会見日にはドリフトがあるが短命だとする — [T&F](https://www.tandfonline.com/doi/full/10.1080/00036846.2024.2322573)。QuantSeeker の SPY 戦略（会合前日の引けで買い、会合最終日の引けで売る。1993–2024 年、片道 5bp のコスト後）は CAGR 約 4%、Sharpe 0.5–0.6 で、2016–2019 年はフラット、その後は強め。VIX の高い四分位ではドリフトが有意に強く、低 VIX ではほぼゼロ — [QuantSeeker](https://www.quantseeker.com/p/trading-the-fed-the-pre-fomc-drift)

### Inferences
- 実装ルール案：US500 を会合前日の引け（または 2 日前の引け）で買い、声明発表直前（14:00 ET の 1 分前）か会合日の引けで決済する。VIX が高い（例：20 超）ときだけ取引する。年 8 回なので、パフォーマンスはほぼ検証不能。ポートフォリオの補助ルールにとどめるのが妥当。
- 保有は 1 泊で、financing は 1 日分（年率 5–8% 程度の CFD 金利と仮定すると、1 泊あたり約 2bp）。期待値が 20bp 前後あれば残るが、ばらつきは大きい。

### Gaps
- 2020–2026 年の FOMC 前ドリフトを査読付きで測った数値は見つからなかった（QuantSeeker の読者コメントの年別値は未検証）。

---

## Q4. 株価指数：暦効果（月替わり、祝日前、オプション満期）

### Takeaway
月替わり（TOM）効果は米国で過去 20 年に消えたとする査読論文が 2024 年に出ている。祝日前効果も最近のデータでは弱い。OPEX 週の効果も、主に商用バックテストの話で、近年は劣化している。いずれも単独で優位性を見込むのは難しい。

### Cited Findings
- 「The disappearing turn-of-month effect」（Finance Research Letters, 2024）。米国株では過去 20 年で TOM 効果が完全に消えた。2001 年以降の取引コスト低下による裁定が原因としている — [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S1544612324014909)
- Atlanta Fed の WP：S&P 500 先物の TOM 効果は 1990 年以降消えた — [FedInPrint](https://www.fedinprint.org/item/fedawp/13394/original)。一方で CXO の要約は「月末最終日の引けで買い翌日売り（+1）だけが全期間でネットの収益性を持つ」とし、見解が割れている — [CXO](https://www.cxoadvisory.com/calendar-effects/stock-index-futures-calendar-effects/)
- Quanterlab の SPY 研究（2006–2026 年、非査読）：月末最終日前の 4 営業日が 1 日 0.095% で、平均日 0.049% の約 2 倍 — [Quanterlab](https://quanterlab.com/research/in-search-of-the-market-s-payday)
- 祝日前効果：歴史的には Pettengill（1962–1986 年）や Fosback（1928–1975 年）で確認されている。最近のデータでは CXO（Labor Day, 1950–2024 年）が「ノイズが支配的」とし、Quantpedia も「暦効果は小型株指数以外では縮小」としている — [Quantpedia](https://quantpedia.com/strategies/pre-holiday-effect), [CXO Labor Day](https://www.cxoadvisory.com/calendar-effects/stock-returns-around-labor-day/)
- OPEX：Stivers-Sun は 1983–2008 年の S&P 500 で満期週のリターンが高いと報告（1983 年以前は効果なし）— [CXO drill-down](https://www.cxoadvisory.com/calendar-effects/option-expiration-week-stock-return-drill-down/)。QuantifiedStrategies では満期週の CAGR が 2%、週平均 +0.2%、満期日当日は弱く、近年は効果が弱いとしている — [QuantifiedStrategies](https://www.quantifiedstrategies.com/options-expiration-week/)

### Inferences
- TOM の実装ルール案（検証用）：US500 を月末から数えて 4 営業日前の引けで買い、翌月第 3 営業日の引けで売る。保有は約 6 泊なので financing は約 10–15bp かかる（年率 6% の仮定）。効果が 1 日 5bp 程度しか上乗せされないなら、ほぼ相殺される。
- 祝日前効果と OPEX 効果は、単独戦略ではなく、他戦略のフィルター（例：その日は売りシグナルを控える）として使うのが限界だろう。

### Gaps
- 2018–2026 年の TOM／OPEX を指数先物で測った査読研究は見つからなかった。0DTE オプション普及後の満期効果も未調査。

---

## Q5. 株価指数：短期平均回帰（RSI(2)、IBS）、ボラティリティ管理、トレンドフォロー

### Takeaway
日足の短期平均回帰（RSI(2) ＜ 10 で買う、IBS ＜ 0.2 で買う）は、米国指数で 1990 年代から続いている「最も劣化しにくい」種類のパターンとされる。ただし根拠は商用・ブログのバックテストが中心で、査読付きの OOS 検証は乏しい。ボラティリティ管理は、市場ファクター単体（指数）では OOS でもわずかに有用という見方と、無効という見方が混在している。

### Cited Findings
- IBS（Pagonidis 2013, NAAIM）：IBS =（終値−安値）/（高値−安値）。IBS ＜ 0.2 の日の翌日平均は +0.35%、IBS ＞ 0.8 では −0.13%。効果は 1990 年代初頭から一貫して見られ、米国指数 ETF では高出来高の日にのみ効く、とされる。年率 30% 超というアルファはコスト前 — [NAAIM PDF](https://www.naaim.org/wp-content/uploads/2014/04/00V_Alexander_Pagonidis_The-IBS-Effect-Mean-Reversion-in-Equity-ETFs-1.pdf), [ResearchGate](https://www.researchgate.net/publication/256065291_The_IBS_Effect_Daily_Mean_Reversion_of_Equity_Indices)。国別 ETF への適用（arXiv 2023、学生レベルの報告）— [arXiv](https://arxiv.org/abs/2306.12434)
- RSI(2)：SPY の 1993 年以降のバックテストで、RSI(2) ＜ 10 で買い、＞ 80 で売るルールは年率約 9%、最大 DD 34%、市場滞在率約 28%（QuantifiedStrategies、非査読）— [QuantifiedStrategies](https://www.quantifiedstrategies.com/rsi-2-strategy/)。S&P 500 の 1990–2024 年を対象にした個人の Python 検証は、2010 年の書籍出版後も効果が持続したとしつつ、単独戦略には不向きと結論 — [Trade2Win](https://www.trade2win.com/threads/backtest-results-for-connors-rsi2-strategy.242688/)
- ボラティリティ管理：
  - Cederburg ら（2020）は 103 の株式ポートフォリオで、in-sample の利益が OOS ではほぼ再現されないと報告 — [ResearchGate](https://www.researchgate.net/publication/341087390_On_the_performance_of_volatility-managed_portfolios)
  - Barroso-Detzel（2021）は、取引コストで市場ファクター以外の利益は消え、市場ファクターでの利益も高センチメント期に限られるとする — [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0378426621001576)
  - DeMiguel ら（2024 JF）は、条件付きマルチファクターなら OOS・コスト後でも有効とする — [Wiley](https://onlinelibrary.wiley.com/doi/full/10.1111/jofi.13395)
- トレンド（TSMOM）：金属カテゴリの一般的な TSMOM は Sharpe 0.16（Uhl 2025, Review of Financial Economics）。投機筋のポジションと TSMOM の一致度が高いほど成績が落ちる — [Wiley](https://onlinelibrary.wiley.com/doi/full/10.1002/rfe.1228)

### Inferences（実装ルール案）
- IBS／RSI(2) の日足版：US500・US100・US30 の D1。条件は「終値 ＞ 200 日 SMA」かつ「IBS ＜ 0.2」または「RSI(2) ＜ 10」。引けで買い、「終値 ＞ 前日高値」「RSI(2) ＞ 70」「最大 5 日」のいずれかで手仕舞う。
  - SL 必須の制約があるため、SL は 3–4×ATR(D1) 程度になる。H1 ATR×4 のガードを超える可能性が高いので、ガードの例外設定か、D1 ベース ATR の許容が必要。ただし設定を勝手に変えないこと。新しい制約は既定オフにする。
  - 1 トレードの平均保有は 2–4 泊で、financing は 1 トレードあたり約 3–7bp（年率 6% の仮定）。平均期待値が 30–50bp あれば残る計算になる。
  - CFD の日足の「終値」は 23:00 前後のブローカー日付で切られるため、現物の 16:00 ET の引けとは一致しない。シグナルは 16:00 ET の M5 バーから自前で日足を合成するのが望ましい。
- ボラティリティ・ターゲティングは、新しいエッジではなくポジションサイジングの補助（リスク平準化）として位置づけるのが無難。

### Gaps
- 2018–2026 年の RSI(2)／IBS を年別に示した信頼できる出典は見つからなかった（2020 年と 2022 年の下落局面の挙動など）。
- GER40 や JP225 での IBS 検証も見つからなかった。

---

## Q6. 金（XAUUSD）：時間帯、ロンドン・フィックス、FOMC、トレンド

### Takeaway
金は「オーバーナイト（アジア時間）で上がり、日中（ロンドン〜NY の取引時間）で下がる」という傾向が、1985–2012 年の COMEX、ロンドン・フィックス、GLD で一貫して報告されている。2014 年時点で「弱まったが残っている」とされた。しかし 2015 年のフィックス改革以降と、2018–2026 年の検証は査読論文としては見当たらない。Quantpedia は、OHLC データの使い方次第で同種の効果が過大評価される（データの人工物）と警告している。

### Cited Findings
- Blose, Gondhalekar ら（2014, Applied Economics Letters）「Overnight gold returns」：1985–2012 年の COMEX 金で、オーバーナイトは有意に正、日中は有意に負。GLD でも同様で、非対称性は弱まりつつ持続している — [T&F](https://www.tandfonline.com/doi/full/10.1080/13504851.2014.922661)
- Blose, Gondhalekar, Kort（2018, Journal of Economics and Finance 42, 526–549）：COMEX 先物、ロンドン・フィックス現物、金鉱株、金 ETF・CEF のいずれでも、オーバーナイトは正、日中は負。上昇相場でも下落相場でも成り立ち、取引コスト後も経済的に意味があるとする。解釈は「各市場の寄り付きで割高に値付けされる」 — [Springer](https://link.springer.com/article/10.1007/s12197-017-9403-0), [ResearchGate](https://www.researchgate.net/publication/318356821_Overnight_versus_day_returns_in_gold_and_gold_related_assets)
- Quantpedia：GDX のオーバーナイトリターンは年率 30% 超に見えたが、1 分足で 9:31 の約定で検証すると大幅に縮小した。OHLC の始値をそのまま使うことの危険を示す例 — [Quantpedia](https://quantpedia.com/dangers-of-relying-on-ohlc-prices-the-case-of-overnight-drift-in-gdx-etf/)
- ブローカー資料（VT Markets、マーケティング）は「2026 年の金はアジア時間で上昇、米国時間で下落」としているが、非査読で方法も不明 — [VT Markets](https://vtmarkets.com/en-ca/discover/xauusd-vs-xauusd247-trading-hours-costs-and-volatility)
- 中国の金・銀先物では「夜間効果」が報告されている（2025 年、Pacific-Basin 系）— [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S1044028325000110)
- ロンドン PM フィックス：Caminschi-Heaney（2014, Journal of Futures Markets）は 2007–2012 年の GC 先物と GLD で、フィックス開始直後に出来高とボラティリティが急増し、開始後 4 分間に情報優位の取引で有意なリターンがあったとする（情報漏洩）。二次情報によれば、15:01 以降の先物の方向とフィックスの方向の一致率は 69.9%、5 分以内に 80% に達する。ただし 2015 年に LBMA Gold Price（電子オークション）へ移行しており、サンプルは改革前 — [Wiley](https://onlinelibrary.wiley.com/doi/10.1002/fut.21636), [AUT 新フィックス研究](https://acfr.aut.ac.nz/__data/assets/pdf_file/0011/29792/A-Aspris-Towards-a-New-Fix-Version-005-Submit.pdf)
- CBS の修士論文（2019）は、操作が疑われた年にフィックス時刻前後で金価格が下落していたと報告（非査読）— [CBS thesis](https://research.cbs.dk/files/59803241/651553_Thesis_Contract_13383.pdf)
- FOMC と金：5 分足では、予想より緩和的な驚きに対する反応のほうが引き締めより大きく、調整は 5 分以上続く（短期の非効率の可能性）— [ScienceDirect 2024](https://www.sciencedirect.com/science/article/abs/pii/S1057521924004186)。金の「FOMC 前ドリフト」を確認した研究は見つからなかった
- 金のトレンド：2015–2025 年の金先物で Sharpe 2.88、年率 43% とする arXiv 論文があるが、未査読で訓練期間にキャリブレーションしており、信頼性は低い — [arXiv 2511.08571](https://arxiv.org/html/2511.08571v1)。金属全般の TSMOM は Sharpe 0.16 と弱い（Uhl 2025）— [Wiley](https://onlinelibrary.wiley.com/doi/full/10.1002/rfe.1228)

### Inferences（実装ルール案）
- 金の時間帯ルール（検証用）：XAUUSD の H1。例えば NY 引け後（17:00 ET 前後、ブローカーの日付切り替え直後）に買い、ロンドン寄り（3:00 ET ≒ 8:00 London）前後で手仕舞う。逆に、ロンドン寄りから NY 引けまで売る。
  - 日付切り替えをまたぐと XAUUSD の swap がかかり、ロング側は通常マイナス。金のロング swap は年率で数 % 相当なので、1 泊あたり 1–2bp。
  - したがって「日付切り替え直後に買い、同じ日の中で売る」ように設計すれば financing をゼロにできる。これが CFD での実装上の利点になる。
  - 効果の大きさは 2018 年以降未確認。2024–2026 年の金の大相場（上昇トレンド）の影響を除いて、年別に評価する必要がある。
- ロンドン・フィックス系は、2015 年の改革で前提が崩れている。CFD でフィックス直後数分を取るのはコストとレイテンシーの面で現実的ではない。
- FOMC 発表後の金の継続（5 分以上の調整）は M1–M5 で検証できる余地がある。ただし発表直後は CFD の実質的なスプレッド拡大やスリッページが大きい（ゼロスプレッド口座でも発表時は広がる可能性。未確認）。

### Gaps
- 2015 年以降（特に 2018–2026 年）の XAUUSD のセッション別リターン分解を示す査読研究は見つからなかった。最も重要な欠落であり、自前データでの検証が必須。
- 金の季節性（月別）、実質金利・ドルへの反応の研究は今回の調査では取得できなかった。

---

## Q7. CFD コストとの比較（手数料＋オーバーナイト金利）

### Takeaway
日中で完結する戦略（日中モメンタム、ノイズエリア、金の日中セッション）には financing が発生せず、手数料だけが勝負になる。数日保有の戦略（RSI(2)、IBS、TOM、FOMC）は、1 泊あたり約 1.5–2.5bp（年率 5–9% の CFD 金利を仮定）を期待値から差し引く必要がある。具体的なブローカーの手数料・swap は文献からは得られないので、口座の実数値を使うこと。

### Cited Findings
- 文献内のコストの扱い：
  - QuantSeeker の FOMC 戦略は片道 5bp を仮定 — [QuantSeeker](https://www.quantseeker.com/p/trading-the-fed-the-pre-fomc-drift)
  - Gao らは「妥当な取引コスト後も残る」 — [WP PDF](https://www.smallake.kr/wp-content/uploads/2015/01/SSRN-id2440866.pdf)
  - IBS のアルファはコスト前 — [NAAIM PDF](https://www.naaim.org/wp-content/uploads/2014/04/00V_Alexander_Pagonidis_The-IBS-Effect-Mean-Reversion-in-Equity-ETFs-1.pdf)

### Inferences
- 優先度（2018 年以降のエビデンスとコスト構造から）：
  - (1) 指数の日中モメンタム／ノイズエリア（financing なし。ただし 2025 年以降の劣化報告あり）
  - (2) 日足の短期平均回帰（RSI(2)/IBS ＋ 200 日 SMA フィルター。数泊の financing は許容範囲）
  - (3) 金のセッション別ドリフト（未検証だが、日内で完結するよう設計できる）
  - (4) FOMC 前（高 VIX 時のみ）
  - (5) TOM、祝日前、OPEX、オーバーナイト・ドリフト（いずれも出版後に消えたか、financing で相殺される）
- 既に却下済みの「指数→クロス円の 1 分リード・ラグ」と「FX の動的 MA 反発」とは独立した仮説なので、重複はない。

### Gaps
- cTrader ブローカーの具体的な指数 CFD／XAUUSD の手数料と swap は、文献からは得られない。口座仕様で確認が必要。
