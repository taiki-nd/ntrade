# 公表された優位性の減衰・データスヌーピング補正・個人クオンツの検証手順（Edge decay, data-snooping correction, and validation methodology）

調査日: 2026-10-22。この調査ではツール呼び出しを約20回に絞った。一次資料の PDF（PBO 論文、Pseudo-Mathematics 論文、Hayo-Iwatsubo の DP）は全文を読んだ。それ以外のいくつかは要旨と二次要約に頼っており、該当箇所には注記を付けた。

## Q1. 公表後の減衰と、複製・多重検定補正を経て残るもの（McLean-Pontiff, Harvey-Liu-Zhu, Hou-Xue-Zhang, Jensen-Kelly-Pedersen）

### Takeaway
公表されたアノマリーのリターンは、標本外で約26%、公表後で約58%低下する（McLean-Pontiff）。多重検定に合わせて閾値を t>3（または t>2.78）に上げると、株式アノマリーの65〜82%は複製に失敗する（HXZ）。一方 JKP は、ファクターの大半は複製でき、93か国の新データでも機能すると反論している。両者の違いは手法の選択（マイクロキャップの扱い、ベイズ的なシュリンク）にある。両陣営とも「残ったものも効果量は当初報告よりかなり小さい」という点では一致している。

### Cited Findings
- McLean & Pontiff (2016, JF) は97の予測変数を対象とした。ポートフォリオのリターンは標本外で26%、公表後で58%低い。26%はデータマイニング効果の上限推定と解釈され、残る32%（58−26）は公表を知った投資家の取引による減衰とされる。著者らは「投資家は学術論文からミスプライシングを学ぶ」と結論している。 — [McLean & Pontiff PDF (Ivey)](https://ivey.uwo.ca/media/3775549/pontiff.pdf); [gwern mirror](https://Www.Gwern.net/doc/economics/2016-mclean.pdf)
  - 注: 82変数を使った2013年のワーキングペーパー版では「公表後の減衰は約35%」と報告されており、報道（Fortune 等）はこの旧版の数字を引用している。 — [Fortune 2012](https://fortune.com/2012/11/02/academics-academia-destroys-stock-picking-strategies)
- Harvey, Liu & Zhu (2016, RFS) は313本の公表論文と63本のワーキングペーパーから、316のファクターを収集した。累積的なデータスヌーピングを考えると t≥2 は低すぎるとし、新ファクターには t>3.0（p<0.0027 相当）を要求すべきだと提案した。 — [NBER w20592](https://nber.org/papers/w20592); [CXO summary](https://www.cxoadvisory.com/big-ideas/taming-the-factor-zoo/)
- Hou, Xue & Zhang (2020, RFS, "Replicating Anomalies") は452のアノマリーを扱った。マイクロキャップの影響を抑えるため NYSE ブレークポイントと時価加重を使っている。結果は、|t|≥1.96 を満たせないものが65%、多重検定の閾値2.78を課すと失敗率は82%。取引摩擦系のアノマリーは96%が失敗した。複製できたものも、経済的な大きさは当初報告よりかなり小さい。 — [IDEAS/RePEc RFS 2020](https://ideas.repec.org/a/oup/rfinst/v33y2020i5p2019-2133..html); [NBER w23394](https://www.nber.org/papers/w23394.pdf)
- Jensen, Kelly & Pedersen (2023, JF 78(5), 2465–2518) はベイズ的なファクター複製モデルを用いた。結論は四つある。(1) 大半のファクターは複製できる。(2) ファクターは13のテーマに集約され、その多くは接点ポートフォリオの有意な構成要素になる。(3) 93か国の新データでも大半は標本外で機能する。(4) ファクターの数が多いことは、証拠を弱めるのではなく強める。 — [NBER w28432](https://www.nber.org/papers/w28432)
  - 「82%が複製可能、悲観派の35%と対照的」という数字は二次要約（Alpha Architect）由来で、要旨には出てこない。引用する際は本文で確認すること。 — [Alpha Architect](https://alphaarchitect.com/is-there-a-replication-crisis-in-finance/)

### Inferences
- 見積もりの目安: 公表済みの「エッジ」について、バックテストの Sharpe やリターンは少なくとも半分に割り引いて考えるべきである。根拠は McLean-Pontiff の公表後 −58% と、HXZ の「複製できても効果は小さい」という指摘。
- JKP の楽観的な結論は、多数の株式銘柄による横断面ファクターを分散保有した場合の話である。単一銘柄の CFD を少数の設定で売買する個人には、そのまま当てはまらない。先物・FX で同じ規律に当たるのは、複数市場・複数期間で同じ符号が出ること（後述の trend/carry の証拠）である。
- 公表後の減衰には「裁定資本の流入」という機構がある。したがって、容量が小さい・執行が難しい・リスクを取る必要があるエッジほど残りやすい（Q6 で詳述）。

### Gaps
- McLean-Pontiff 本文にあるアノマリー種類別（流動性が高い／低い、裁定コストの高低）の減衰差は、今回の取得範囲では数値を確認できなかった。
- 先物・FX に限った「公表後減衰」の体系的な研究は見つからなかった。FX テクニカルの時間的減衰は Q2 の Neely-Weller-Ulrich で代替する。

## Q2. データスヌーピングに頑健な検定にかけたテクニカル売買ルールの収益性（STW 1999, Hansen SPA, Romano-Wolf, Hsu-Hsu-Kuan 2010, Park-Irwin 2007, FX）

### Takeaway
テクニカルルールの超過収益は1970〜80年代の FX や先物では本物だった（スヌーピングを補正しても残る）。ただし単純な MA やフィルタールールは1990年代初頭までに消えた。Reality Check や SPA を通っても、別期間の標本外では有意性を失う例が典型的に見られる。新興・低効率の市場でのみ残るという結果が多い。

### Cited Findings
- Sullivan, Timmermann & White (1999, JF 54(5), 1647–1691) は、White の Reality Check（ブートストラップ）を使って、全ルール宇宙（7,846ルール）にわたるデータスヌーピングを補正した。DJIA の100年分のデータでは、コスト控除前で補正後も有意だった。しかし1987〜1996年の標本外では、最良ルールの年率8.63%（コスト控除前）は統計的に有意でなかった。 — [LSE Research Online](https://researchonline.lse.ac.uk/id/eprint/119144); [FMG DP303](https://www.fmg.ac.uk/sites/default/files/2020-11/dp303.pdf); OOS の数値は二次引用 [AgEcon cp05pa01](https://ageconsearch.umn.edu/record/19039/files/cp05pa01.pdf)
- Hansen (2005, SPA) は、悪いルールや無関係なルールを多く含めると Reality Check の検出力が低下し、ゼロにもなりうると指摘した。SPA はこの問題を緩和する。 — 二次引用 [EFMA 2008 Bajgrowicz](https://www.efmaefm.org/0EFMAMEETINGS/EFMA%20ANNUAL%20MEETINGS/2008-Athens/papers/Bajgrowicz.pdf)
- Hsu, Hsu & Kuan (2010, J. Empirical Finance 17(3), 471–484) は SPA の段階的版（Step-SPA）を提案し、Romano-Wolf の stepwise RC より検出力が高いことを示した。成長市場・新興市場の指数では有意な予測力があったが、ETF 導入後は証拠が弱まった。 — [HKU repository](https://repository.hku.hk/handle/10722/141768); [arXiv 1504.06397 (citing paper)](https://arxiv.org/pdf/1504.06397)
- Park & Irwin のレビュー（2004年 WP 版）では、近年の研究92本のうち58本が肯定的、24本が否定的、10本が混合だった。テクニカル戦略は少なくとも1990年代初頭まで、様々な投機市場で一貫して経済的利益を生んだとされる。同時に、データスヌーピング、事後的なルール選択、リスクとコストの推定の難しさという問題が指摘されている。 — [AgEcon AgMAS04_04](https://ageconsearch.umn.edu/record/37487/files/AgMAS04_04.pdf); 2007 JES 版 [RePEc](https://ideas.repec.org/a/bla/jecsur/v21y2007i4p786-826.html)
  - 注: よく出回る「95本中56本」という数字は、今回は確認できなかった。
- Neely, Weller & Ulrich (2009, JFQA 44, 467–488) によると、FX テクニカルの1970〜80年代の超過収益はデータマイニングの産物ではなく本物だった。しかしフィルタールールと MA ルールの利益機会は1990年代初頭までに消えた。研究の少ないルールのリターンも低下したが、完全には消えていない。これは適応的市場仮説と整合し、EMH とは整合しない。 — [St. Louis Fed WP 2006-046](https://files.stlouisfed.org/files/htdocs/wp/2006/2006-046.pdf); [JFQA/Cambridge](https://www.cambridge.org/core/product/9D336CDCA83233819EB5CDD0F4BC0DAA)
- Neely & Weller (2011 survey) は、ドル為替の単純ルールは約15年間リスク調整後プラスだった後に消滅し、その後は複雑なルールがより控えめなリターンを出していると述べる。 — [St. Louis Fed WP 2011-021](https://files.stlouisfed.org/files/htdocs/wp/2011/2011-021.pdf)

### Inferences
- 短期足の FX/CFD で、単純な MA・ブレイクアウト・平均回帰ルールをパラメータ探索しても、主要通貨ペアでは補正後に何も残らないのが文献上の事前分布である。ユーザーが MA 反発などを棄却した結果は、この事前分布と整合している。
- グリッド全体（例: 400設定）を一つの「ルール宇宙」として扱い、Reality Check、SPA、Romano-Wolf 型の検定を回すのが標準である。ただし無意味な設定を大量に含めると RC の検出力が落ちる。宇宙は事前に経済的理由で絞り、SPA か Step-SPA を使うのが望ましい。

### Gaps
- 2015年以降の主要 FX ペアや XAUUSD のイントラデイテクニカルについて、SPA や Step-SPA を使った最新の体系的研究は、今回の探索範囲では見つからなかった。

## Q3. バックテスト過学習の定量化: PBO/CSCV、Deflated Sharpe Ratio、MinBTL、Harvey-Liu ヘアカット、実装可能な閾値

### Takeaway
試した設定数 N を必ず記録し、選んだ戦略の Sharpe をその N で割り引くのが核心である。具体的には次の三つを使う。(1) 選択プロセス全体の PBO を CSCV で測る（論文の慣行的な棄却基準は PBO>0.05）。(2) DSR（N・歪度・尖度を考慮）が95%以上か見る。(3) バックテスト長が MinBTL を満たすか確認する（N=45 なら5年、N=400 なら約12年が上限目安）。

### Cited Findings
- **PBO / CSCV**（Bailey, Borwein, López de Prado, Zhu, J. Computational Finance 2015/2017）
  - PBO の定義は「IS で最適として選んだ設定が、OOS で N 設定の中央値を下回る確率」。CSCV の手順は、T×N の性能行列 M を S 個の等サイズブロックに分け、S/2 個の組合せ全てを IS、その補集合を OOS として、ロジットの分布を作るというもの。
  - 論文は、標準的な Neyman-Pearson の慣行に従えば PBO>0.05 のモデルは棄却するのが通例だと述べている。
  - S=16 なら12,780通りの組合せになる。4年の日次データなら S=16 は四半期ブロックに相当し、系列相関の構造も保たれる。6年超なら S=24（約270万ロジット）、四半期効果を見るなら S=12（924ロジット）が例示されている。
  - PBO 以外の指標として、OOS 性能劣化（IS が高いほど OOS が低い傾向）、OOS 損失確率、確率的優越（ランダムに選ぶより良いか）が挙げられている。
  - PBO を選択の目的関数にしてはいけない。
  - — [Bailey et al. PBO (carmamaths)](https://carmamaths.org/jon/backtest2.pdf); [SSRN 2326253](https://papers.ssrn.com/abstract=2326253)
- **hold-out が不十分な理由**（同論文）
  - hold-out データは事前に見られがちで、研究者は OOS 期間の市場の動きを知っている。
  - 小標本では推定誤差の分散が大きく、どの区間を hold-out にしたかで結論が変わる。
  - Weiss-Kulikowski を引いて、週次売買なら20年未満のバックテストに hold-out は使うべきでないとしている。
  - 最も重要なのは、hold-out が試行回数 N を考慮しないこと。
  - — [Bailey et al. PBO](https://carmamaths.org/jon/backtest2.pdf)
- **MinBTL**（Bailey, Borwein, López de Prado, Zhu, "Pseudo-Mathematics and Financial Charlatanism", Notices of the AMS 2014）
  - 真の Sharpe が0の独立な戦略 N 個から、IS で Sharpe が E[max_N] のものを誤って選ばないために必要な年数は、MinBTL ≈ ((1−γ)Z⁻¹(1−1/N) + γZ⁻¹(1−1/(Ne)))² / E[max_N]² < 2 ln N / E[max_N]²。
  - 5年分のデータなら、独立な試行は45個以下に抑えるべきである。それを超えると、IS の Sharpe が1でも OOS の期待 Sharpe は0という状況がほぼ確実に起きる。
  - 2年のバックテストでは、独立な試行7個で期待最大 IS Sharpe が1に達する。
  - 試行が独立でなければ、PCA などで実効的な独立試行数を推定する。
  - MinBTL は必要条件であって十分条件ではない。試行数 N を報告しないバックテストは、過学習リスクを評価できない。
  - — [Pseudo-Mathematics PDF (carmamaths)](https://carmamaths.org/jon/backtest.pdf)
- **DSR**（Bailey & López de Prado 2014, JPM）
  - DSR は二つの水増し要因を補正する。多重検定下の選択バイアスと、リターンの非正規性である。
  - DSR は PSR の一種で、閾値 SR₀ を「N 試行の期待最大 Sharpe」に置いたもの。SR₀ = √V[SR_n]·((1−γ)Φ⁻¹(1−1/N) + γΦ⁻¹(1−1/(Ne)))。
  - PSR = Φ((SR̂−SR₀)√(T−1) / √(1 − γ₃SR̂ + (γ₄−1)/4·SR̂²))。γ₃ は歪度、γ₄ は raw kurtosis（正規分布で3）。
  - 実装上の注意が二つある。年率化した Sharpe ではなく期間あたりの Sharpe を使うこと（年率化すると DSR は1付近に張り付く）。尖度は超過尖度でなく raw kurtosis を使うこと。
  - 「DSR>0.95」という基準は原論文ではなく二次資料の慣行である。
  - — [SSRN 2460551](https://papers.ssrn.com/abstract=2460551); [Wikipedia: Deflated Sharpe ratio](https://en.wikipedia.org/wiki/Deflated_Sharpe_ratio); [fynance docs](https://fynance.readthedocs.io/en/latest/generated/fynance.research.deflated_sharpe_ratio.html)
- **Harvey & Liu "Backtesting"**（JPM 2015, 41(1), 13–28）
  - Sharpe を t 値に変換し、多重検定（Bonferroni、Holm、BHY）で p 値を補正してから Sharpe に戻す。
  - 業界でよく使われる「一律50%ヘアカット」は重大な誤りだとしている。ヘアカットは非線形で、高い Sharpe は軽く、限界的な Sharpe は重く減額される。
  - 有意とみなすための利益ハードルも提示している。
  - — [Duke PDF](https://people.duke.edu/~charvey/Research/Published_Papers/P120_Backtesting.PDF); [quantstrat profitHurdle](https://rdrr.io/github/braverock/quantstrat/man/profitHurdle.html)

### Inferences
- **ユーザーの約400設定への適用（試算。著者の数値ではない）**
  - 2 ln 400 ≈ 12.0 なので、IS の年率 Sharpe 1.0 を信じるには、独立試行換算で約12年分のバックテストが必要になる。
  - 5年のデータでは、帰無仮説下の期待最大 Sharpe は約 √(12/5) ≈ 1.55 になる。
  - DSR の SR₀ を N=400 で計算すると、約 2.98×σ(SR)。年率 Sharpe 推定量の標準誤差 ≈ 1/√年数 とすれば、5年で約1.33、10年で約0.94 が「運だけで出る最良値」の目安になる。
  - ただしグリッドの隣接設定は強く相関しているので、実効 N は400よりかなり小さい。PCA やクラスタリングで実効 N を見積もり、その値と名目の400の両方で DSR を報告するのが誠実である。
- **実装チェックリスト（Rust バックテスタ向け、閾値は文献の慣行と上記試算に基づく）**
  1. 全試行の OOS／IS の期間リターン系列を保存し、T×N 行列 M を作る（破棄した戦略を含む全試行の N を数え続ける）。
  2. CSCV（S=16、期間が長ければ S=24）で PBO を計算し、PBO>0.05 なら棄却する（論文の慣行）。実務上は0.05〜0.2を灰色帯とする選択肢もあるが、その根拠となる文献は見つけていない。
  3. 選んだ戦略の DSR を、期間あたりの Sharpe、raw kurtosis、実効 N で計算し、DSR≥0.95 を要求する。
  4. バックテスト年数 ≥ 2 ln(N_eff) / SR_target² を確認する。
  5. 取引回数の最低基準: Sharpe の標準誤差は概ね 1/√(独立観測数) で縮む。取引回数が数十回だと DSR がほぼ確実に不合格になるので、自動的にふるい落とされる。「最低○回」という固定閾値に普遍的な文献の根拠は見つからなかった（Gaps 参照）。
  6. 最後に、一度だけ使う真の hold-out（バックテストに一切触れていない期間、またはフォワードのペーパー取引）で、符号と大きさが合っているかを確認する。

### Gaps
- 「最低取引回数（例: 100回、200回）」の普遍的閾値について、査読付きの根拠は見つからなかった。DSR や PSR が取引回数（T）を通じて内生的に扱うので、それで代替するのが筋である。
- DSR の原論文 PDF は SSRN が403で取得できなかったため、数式はウィキペディアと実装ドキュメントで照合した。

## Q4. ウォークフォワード分析のベストプラクティスと落とし穴、試行回数の扱い

### Takeaway
ウォークフォワード（WF）は時系列順を守るが、得られる経路が1本なので推定分散が大きい。WF のパラメータ（窓長、再最適化頻度）自体も試行回数に含まれる。対策として、CPCV（purge と embargo 付き）で多数の OOS 経路の分布を得て、それとは別に多重検定補正（DSR、PBO）を必ず併用する。

### Cited Findings
- WF は単一の時系列経路に依存するため、性能推定の分散が大きい。CPCV は多数の train/test 分割から OOS 性能の分布を作る。 — [Wikipedia: Purged cross-validation](https://en.wikipedia.org/wiki/Purged_cross-validation)
- CPCV（N グループ、k テストグループ）からは C(N,k) 個の分割が作られる。例として N=6、k=2 で15。 — [claude-trading-skills walk-forward doc（非学術、参考程度）](https://claudeskills.info/skills/agiprolabs/claude-trading-skills/walk-forward-validation/)
- purge は、ラベル期間がテスト期間と重なる訓練サンプルを除く処理。embargo は、テストブロックの直後に空白を置き、系列相関による漏洩を防ぐ処理。ただしこれらは、未来情報を含む特徴量による漏洩や、多数の変種を試すことによる過学習には対処しない。 — [LuxAlgo purged CV](https://www.luxalgo.com/library/concept/purged-cross-validation/)
- 応用例: 10グループ・8テストフォールドの CPCV で36経路と45訓練組合せを作り、21日の purge と embargo を設定した研究がある。 — [arXiv 2407.17645](https://arxiv.org/pdf/2407.17645)
- hold-out（WF の各 OOS 窓を含む）は試行回数 N を考慮しない。研究者が OOS 期間の結果を見て設定を修正すると、その OOS は IS になる。 — [Bailey et al. PBO](https://carmamaths.org/jon/backtest2.pdf)

### Inferences
- **よくある落とし穴**
  - WF の結果を見てから窓長や再最適化周期、グリッドを変える。これは OOS の再利用であり、N を増やしている。
  - WF の OOS をつないだ曲線を、さらに複数の戦略族の中から選ぶ。これも選択バイアスになる。
  - コストを過小評価する。特に CFD のスプレッドや手数料、スワップ、スリッページ、約定拒否。
  - 指値の約定をバー内の価格だけで判定する（楽観バイアス）。
  - 時間足をまたいで特徴量が未来の情報を参照する。
- **推奨**
  - WF の設計（窓長など）は事前に固定する。
  - WF で得た OOS 系列にも DSR を適用する。その際の N は「戦略族の数 × WF 設定の変種数」まで数える。
  - さらに CPCV で、OOS Sharpe の分布の下側（例: 10パーセンタイル）が0より上かを見る。
  - 「OOS で一度でも見た期間」はもう検証データではないと記録する。

### Gaps
- WF と CPCV を直接比較し、どちらが真の将来性能をよく予測するかを示す査読付き研究（López de Prado らのシミュレーション比較）は、今回の検索では原典に到達できなかった。

## Q5. 個人 CFD/FX トレーダーの成績とその原因（ESMA、Heimer-Simon、Hayley-Marsh、日本の研究）

### Takeaway
EU 規制当局の分析では、個人 CFD 口座の約74〜89%が損失を出しており、口座あたりの平均損失は1,600〜29,000ユーロだった。経験を積んでも勝てるようにはならない（Hayley-Marsh）。損失の要因は、取引コスト、高レバレッジ、過剰取引、社会的伝播による活発な取引である。

### Cited Findings
- ESMA は2018年、各国当局の分析として、個人 CFD 口座の74〜89%が損失、平均損失は1,600〜29,000ユーロと公表した。そのうえで MiFIR 第40条に基づき、以下を導入した。
  - レバレッジ上限30:1〜2:1（原資産のボラティリティに応じる）
  - 口座ごとのマージンクローズアウト
  - 負の残高の保護
  - 業者固有の損失口座比率の表示（直近12か月、四半期ごとに更新、取引コストを含む）
  - — [ESMA press release](https://www.esma.europa.eu/node/84933); [ESMA notice of PI decisions](https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf); 74–89% の表記は [Investment Executive](https://www.investmentexecutive.com/?p=348240)
- Hayley & Marsh (2016, JIMF 64, 16–38, "What do retail FX traders learn?") の結果は次のとおり。
  - 大手プラットフォームのデータで、取引が上手くなる学習の証拠はない。
  - 学ぶのは自分の能力についてであり、負けた日の後は退出、取引の縮小、頻度の低下が起きやすい。
  - 非常に経験豊富なトレーダーも一貫して損失を出す。
  - 退出を制御すると、ベテランほど成績がわずかに悪化する。
  - — [RePEc](https://ideas.repec.org/a/eee/jimfin/v64y2016icp16-38.html)
- Heimer & Simon ("Facebook Finance", 2012 WP) は、ソーシャルネットワーク型の個人 FX プラットフォームを分析した。
  - 成功したトレーダーは、最も成功していないトレーダーより約50%多く取引について語る。
  - それを聞いた者は翌週の取引が約20%増える。
  - 二次報道によれば、11万件超の取引で個人 FX トレーダーは平均で週3%の損失を出していた。
  - — [CXO Advisory summary](https://www.cxoadvisory.com/?p=20624); 週3%は [Jason Zweig/WSJ 経由](https://jasonzweig.com/?p=2530)（二次）
- 同じ二次報道では、NFA を引用して「個人口座の72%が損失、口座の平均寿命は4か月」とされる。NFA の一次資料では確認していない。 — [Jason Zweig](https://jasonzweig.com/?p=2530)
- 日本: Hayo & Iwatsubo（神戸大 DP1909, 2019。後に MDPI Sustainability 2022）は、2018年の金融先物取引業協会の調査（日本の FX 証拠金取引者1,000人、2017年の損益を自己申告）を使った。
  - 高齢、戦略がない、高レバレッジ、長期志向は成績の悪化と関連していた（1標準偏差あたり、レバレッジで約−4.3万円、戦略なしで収益率が12pp低い）。
  - 取引額が大きい、ファンダメンタルズを重視する、所得が高いことは、成績の向上と関連していた。
  - 客観的な金融知識や過信は、成績を説明しなかった。
  - なお、この標本の平均損益は自己申告でプラスだった（自己申告バイアスに注意）。
  - 日本は世界の個人スポット FX 取引の36%を占める。
  - — [Kobe Univ DP1909](https://www.econ.kobe-u.ac.jp/wp/wp-content/uploads/2023/06/1909.pdf)
- 日本の個人 FX 証拠金取引の取引高は、2022年に1京円を超え過去最高となった。 — [BoJ Review 2023-E-7](https://www.boj.or.jp/en/research/wps_rev/rev_2023/rev23e07.htm)

### Inferences
- **エッジが消えるコストの構造**: 個人 CFD の損失は、総リターンがゼロ近辺のところからコスト（スプレッドまたは手数料、スワップ、スリッページ）が引かれることで生じる。
  - 取引回数が多いほど、コストは試行回数に比例して積み上がる。
  - ゼロスプレッド＋手数料型の口座でも、往復手数料 × 取引回数がそのまま期待値から差し引かれる。
  - 短期戦略ほど「1取引あたり期待値 ÷ 往復コスト」の比が小さくなり、脆くなる。
  - バックテストでは、コストを2倍にしたストレステストで符号が維持されることを確認すべきである（一般的な実務慣行。特定文献の閾値ではない）。
- **行動面**: 損失後の退出（Hayley-Marsh）と社会的伝播（Heimer-Simon）は、裁量取引の問題である。ルールベースの自動執行は、この部分を排除できる。ただし統計的なエッジが無いことは補えない。

### Gaps
- Hayley-Marsh のその他の論文（例: 個人 FX のディスポジション効果）は、今回は確認していない。「Hayley-Marsh が retail FX の損失率を示した」という具体的な数値は見つからなかった。
- cTrader 系ブローカー固有の損失口座比率は、各社の開示ページを個別に確認する必要がある（未調査）。
- ESMA 措置のその後の改定状況は確認していない。

## Q6. 構造的に説明されるエッジ（リスクプレミアム・フロー）と純統計パターンの持続性

### Takeaway
長期にわたり複数市場で残っている先物・FX のエッジは、トレンド（時系列モメンタム）とキャリーのように、リスク負担（キャリーのクラッシュリスク）や行動・制度的な理由で説明できるものである。純粋な統計パターン（単純な FX テクニカル）は、発見・公表とともに消えた。

### Cited Findings
- Hurst, Ooi & Pedersen ("A Century of Evidence on Trend-Following Investing", JPM 2017, 44(1)) は、1880年以降、4資産クラス67市場で時系列モメンタム（ボラティリティ目標10%）を検証した。110年以上一貫して利益が出ており、伝統資産との相関は低く、株式の弱気相場で分散効果があった。サブ期間、市場状態、経済状況での頑健性も確認されている。 — [AQR](https://www.aqr.com/Insights/Research/Journal-Article/A-Century-of-Evidence-on-Trend-Following-Investing); [PDF](https://fairmodel.econ.yale.edu/ec439/hurst.pdf)
  - 「手数料2/20控除後の Sharpe 約0.4」は二次ブログの数字で、未検証。
- Brunnermeier, Nagel & Pedersen ("Carry Trades and Currency Crashes", NBER Macro Annual 2008) によると、キャリートレードの高金利通貨には負の歪度（クラッシュリスク）がある。原因は、リスク選好や資金流動性が低下したときに起きる突然の巻き戻しである。VIX が上昇すると歪度が悪化する。つまりキャリーの収益は、クラッシュリスクを負担することへの対価と解釈できる。 — [NBER w14473](https://www.nber.org/papers/w14473)
- FX の単純テクニカルは1990年代初頭までに消滅した（純統計パターンの減衰例）。 — [Neely-Weller-Ulrich](https://files.stlouisfed.org/files/htdocs/wp/2006/2006-046.pdf)
- McLean-Pontiff: 公表後の減衰は、投資家が論文から学んで裁定することで生じる。裁定が容易なものほど消えやすい。 — [McLean & Pontiff](https://ivey.uwo.ca/media/3775549/pontiff.pdf)

### Inferences
- 構造的なエッジが残る理由は、そのリターンが誰かのリスク負担への対価か、制約のある参加者（ヘッジャー、リバランサー、レバレッジ制約のある投資家）からの移転だからである。裁定すれば必ずリスクやコストを負うので、完全には消えない。純統計パターンにはそのような「支払い手」がおらず、知られれば消える。
- **ユーザーへの含意（推論）**
  - 新しい戦略案を作る前に、「誰が、なぜこのリターンを払うのか」という問いを書面で答えられることを、事前のフィルターにするとよい。棄却済みの五十日仲値やリードラグは、フローによる説明はあるが容量が小さく、既に知られている典型である。
  - 単一の CFD 口座で実行できる構造的エッジの候補は、複数市場（FX、指数、金）をまたぐ低頻度の時系列トレンドとキャリーである。ただしリターンは小さく（Sharpe は1未満）、ドローダウンも長い。この場合も DSR、PBO、MinBTL で検証すること。
  - CFD のスワップ（ファイナンシングコスト）がキャリー収入を相殺しうるので、キャリーを使うなら実際のスワップ条件で計算する必要がある。

### Gaps
- ボラティリティリスクプレミアム、リバランスフロー、月末フロー、ヘッジ需要（Moskowitz-Ooi-Pedersen 2012 の TSMOM、Koijen et al. の carry、Etula et al. のリバランス研究など）の具体的な数値は、ツール呼び出しの上限のため今回は取得していない。
- CFD 経由でトレンドやキャリーを実装した場合の、スワップ・手数料控除後の実証研究は見つからなかった。
