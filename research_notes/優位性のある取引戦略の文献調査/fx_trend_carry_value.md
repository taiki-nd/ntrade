# 中期（日足〜月次保有）FX戦略の文献調査: トレンド/TSMOM・キャリー・バリュー・ドル因子・組み合わせ（個人トレーダーが実装する前提）

前提: cTrader のゼロスプレッド+手数料口座（FXの往復コストは約1.5bp）、オーバーナイトでスワップが発生、Rust の足ベース執行。棄却済みの戦略: 8通貨のクロスセクション通貨強弱モメンタム/リバーサル、AUDNZD/EURGBP の平均回帰、動的MA反発。
調査日: 2026-10-22。ツール呼び出しは約17回。いくつかの数値はアブストラクトや二次要約から取っており、その場合は明記した。

## Q1. FXの時系列モメンタム/トレンドフォロー: 効くルックバックと、2010年頃以降の劣化

### 要点
FXの時系列モメンタム（TSMOM）は1970〜2000年代には実在したが、G10通貨単体では2008年以降の成績がほぼ横ばいで、FXはトレンド戦略の中でも最も弱いセクターになっている。文献の標準仕様は1/3/12か月リターンの符号、またはEMAクロスの合成（複数ホライズン）で、市場ごとにボラティリティを揃え、月次〜日次でリバランスする。G10通貨ペアだけ・単一ペアだけで運用した場合、手数料控除前でも Sharpe は0〜0.3程度と見積もるのが妥当。

### 出典付きの知見
- **Hurst-Ooi-Pedersen（AQR）「A Century of Evidence on Trend-Following Investing」**: 1か月・3か月・12か月の TSMOM を等ウェイトで合成。67市場（商品29、株価指数11、債券15、通貨ペア12）が対象。各ポジションは同じボラティリティになるようサイズを決め、毎月集計して、ポートフォリオ全体を事前ボラティリティ10%に合わせる。1880〜2013年の成績は、取引コスト控除後・手数料前で年14.9%、2-and-20 の手数料控除後で年11.2%、ボラティリティ9.7%、手数料控除後 Sharpe 0.77、米国株との相関0.00。FXのユニバースは AUD, CAD, DEM/EUR, JPY, NZD, NOK, SEK, CHF, GBP, USD の通貨フォワード。片道取引コストはAQRの2012年推計に基づき、1993〜2002年は2倍、1880〜1992年は6倍と仮定。著者自身も「将来の Sharpe が0.4程度（過去の半分）でも分散効果に価値がある」と書いている。FX単独の Sharpe はこの資料では確認できなかった。 — [AQR/Hurst-Ooi-Pedersen PDF](https://www.TrendFollowing.com/whitepaper/Century_Evidence_Trend_Following.pdf)
- **Moskowitz-Ooi-Pedersen (2012, JFE)「Time Series Momentum」**: 58の先物/フォワード（通貨を含む）で、12か月リターンの符号に基づく TSMOM と、ボラティリティで逆比例させたサイジングが元論文の仕様。 — [MOP PDF](https://w4.stern.nyu.edu/facdir/lpederse/papers/TimeSeriesMomentum.pdf)（本文の通貨別数値は今回は抽出していない）
- **Rohrbach, Suremann, Osterrieder（ZHAW, SSRN 2949379, 2017）**: Baz et al.（2015, Man/AHL 系の実務仕様）の EMA クロス合成を採用。短期 EMA (8,16,32) と長期 EMA (24,48,96) の3組のクロスオーバーを日次データで計算する。G10、1975〜2017年の結果は以下のとおり（コストは含まない）。
  - 時系列ポートフォリオ: 年2.45%、Sharpe 0.53
  - クロスセクションポートフォリオ: 年0.89%、Sharpe 0.22
  - 時系列ポートフォリオの最大DD（約7%）は2008年12月に始まり、論文時点まで未回復
  - 著者の結論は「G10通貨のモメンタムリターンは消滅した（vanished）」
  - EM通貨のほうが高ボラで Sharpe も高い
  — [Rohrbach et al. PDF](https://assets.super.so/e46b77e7-ee08-445e-b43f-4ffd88ae0a0e/files/9c27aa78-9b14-4419-a53d-bc56fa9d43b2.pdf)
- **Menkhoff-Sarno-Schmeling-Schrimpf (2012, JFE 106(3))「Currency momentum strategies」**: 上位（勝ち）と下位（負け）通貨のクロスセクション・スプレッドは最大で年約10%。従来のリスク因子では説明できず、取引コストで一部が説明される。キャリーとは性質が大きく異なる。収益は高ボラ・高カントリーリスクの小規模通貨に集中しており、強い裁定の限界（limits to arbitrage）があるため容易には取れない、と著者は主張している。 — [BIS WP 366](https://www.bis.org/publ/work366.pdf), [City Univ. 掲載ページ](https://openaccess.city.ac.uk/3296)（数値はアブストラクトに基づく）
- **実務系インデックス（ベンダー記事）**: Bloomberg/S&P の G10 スタイルプレミア指数（ゼロネット通貨）によると、2005年〜2025年3月の FX Momentum は年+5.6%、Sharpe 0.67、FX Carry は年+4.9%、Sharpe 0.60。 — [Resonanz Capital "Style Premia: Are They Still Relevant?"](https://resonanzcapital.com/insights/style-premia-are-they-still-relevant)（インデックス提供元の数値をそのまま引用したもので、独自検証ではない。ZHAW 論文の「G10モメンタム消滅」とは矛盾する。期間と仕様が違い、2008年の大勝ちが含まれる点に注意）
- **ウォークフォワード検証（EPAT 受講生プロジェクト, 2003〜2025年）**: 21の戦略×ペアの組み合わせのうち、OOS Sharpe 0.5 を超えたのは2つで、どちらも TSMOM。全体の OOS Sharpe は0.14で、「薄いが存在する」エッジとされた。 — [QuantInsti](https://www.quantinsti.com/articles/trend-following-strategies-major-currency-markets-epat-project/)（学術査読なし）
- **Macrosynergy（ベンダー）**: マクロ逆風フィルタで FX トレンドを修正すると、長期 Sharpe が0.2から0.32に上がり、2010年代の損失の大半を回避したと主張。 — [Macrosynergy "FX trend following and macro headwinds"](https://macrosynergy.com/research/fx-and-macro-headwinds/)（ページは403で本文未確認。検索スニペットに基づく）
- **マルチアセット TSMOM（FXに限らない）**: 2010年6月以降に Sharpe が大きく低下し、0.7を下回ったという報告。 — [QuantInsti blog](https://blog.quantinsti.com/trend-following-strategy-futures-time-series-momentum-continuous-forecasts-project-jirong-huang/)

### 推論
- 単一ペアや少数ペア（USDJPY, EURUSD, GBPUSD, AUDUSD, EURJPY）での TSMOM は、文献上「G10ではほぼ消えた」と「ベンダー指数では Sharpe 0.6台」のあいだにあり、真値はおそらく0〜0.4。分散が効かない分、単一ペアの Sharpe はさらに低い。
- XAUUSD や株価指数を加えたマルチアセット版にすれば、文献の主張どおり分散効果は回復する。AQR の枠組みはもともと4資産クラスで Sharpe 0.77。FX 単体に期待するのは筋が悪い。
- 実装する仕様は「1/3/12か月の符号を等ウェイト合成」または「EMA (8/24, 16/48, 32/96) クロスの合成」で、各ペアをボラティリティ目標（例: 年10%）でサイズする。月次〜週次リバランスなら回転率が低く、1.5bp のコストはほぼ無視できる。
- 既に棄却したクロスセクション通貨強弱モメンタムとは別物（時系列、長いルックバック、ボラ調整）だが、G10 では両方とも2008年以降に劣化したと報告されている点は共通。

### 未解決
- AQR の資料から FX セクター単独の Sharpe や、2014〜2026年のアップデートは抽出できなかった。AQR "Trend-following: still alive" 系の更新版は未確認。
- 2022年（ドル高・円安）に G10 FX トレンドがどれだけ稼いだかの公的数値は見つからなかった。

## Q2. キャリートレード（クラッシュリスク、フィルタ、2020〜2026年、リテールのスワップコスト）

### 要点
G10キャリーの長期 Sharpe は0.5〜0.8（1976〜2013年）で、負の歪度とクラッシュ（1998年、2008年、2015年、2024年8月）が特徴。ドル中立部分は株式・債券リスク因子で説明されるが、「ドル・キャリー」（米金利と他国金利の中央値の比較で対ドル全通貨を一括売買）は歪度が小さく Sharpe も同等以上。ボラティリティによる調整（高ボラ時に縮小）は文献で支持されているが、OOS で頑健かどうかは不明。リテールCFDではスワップのマークアップがキャリー収益の大半を食う恐れがあり、要検証。

### 出典付きの知見
- **Daniel-Hodrick-Lu (2017, Critical Finance Review 6)**: G10、1976年2月〜2013年8月の基本キャリーの Sharpe は0.78、変形版は最大1.02。
  - ドル中立キャリー（EQ-0$）は負の歪度が強く、リスク因子と相関し、下方リスクを持つ。
  - 動的ドル戦略 EQ-D$ の定義: ドル金利が G10 の中央値金利より低ければ外貨9通貨を等ウェイトでロング（対ドル）、高ければショート。年5.54%、Sharpe 0.68（全期間）、歪度は有意でなく、標準リスク因子とは無相関。
  - 「キャリーのパズル」の多くは条件付きのドルエクスポージャーで説明される。
  — [Daniel-Hodrick-Lu PDF](https://business.columbia.edu/sites/default/files-efs/pubfiles/6378/Daniel.Hodrick.Lu.Carry%20Trade.Critical%20Finance%20Review.2017.pdf)
- **Lustig-Roussanov-Verdelhan「Countercyclical Currency Risk Premia」(NBER w16427, JFE 2014)**: ドル・キャリー（平均フォワードディスカウントが正、つまり外貨金利が平均で米金利より高ければドル売り）は大きな超過リターンを生み、通常のキャリーとは無相関。平均フォワードディスカウントと米鉱工業生産の伸びで、1年先のドルリターン変動の最大25%を予測できる。 — [NBER w16427](https://www.nber.org/papers/w16427.pdf)
- **Jurek（キャリーとクラッシュリスク）**: ドル中立化したうえでオプションでクラッシュリスクをヘッジすると、超過リターンは統計的にゼロと区別できない。2008年に20%の損失があり、全期間の Sharpe は0.57に低下。 — [Jurek PDF](https://www.johnhcochrane.com/s/jurek_currency.pdf)
- **Brunnermeier-Nagel-Pedersen 系 / IMF WP**: キャリーの高い Sharpe はクラッシュリスクの対価と解釈できる。例として1998年に円が2週間で15%上昇した。 — [IMF WP 10/39](https://imf.org/external/pubs/ft/wp/2010/wp1039.pdf)
- **Bekaert-Panayotov「Good Carry, Bad Carry」(JFQA 2020)**: AUD, JPY, NOK といった典型的なキャリー通貨を除いた「良いキャリー」は Sharpe が高く、歪度も小さい。ただし CXO の要約によれば、Sharpe 0.38→0.57 という改善は開始時点からの Sharpe で通貨を選んだ in-sample の数値。 — [JFQA abstract](https://www.cambridge.org/core/journals/journal-of-financial-and-quantitative-analysis/article/abs/good-carry-bad-carry/3BD4883026CA735E1F206588E0B957C4), [CXO summary](https://www.cxoadvisory.com/currency-trading/good-currency-bad-currency)
- **ボラティリティ管理キャリー**: Moreira-Muir (2017, JF) は、高ボラ時にエクスポージャーを縮小するボラ管理の改善例にキャリーを含めている。Mulder-Tims や Dupuy もグローバル FX ボラでキャリーの改善を報告。UZH の研究によると、キャリーとモメンタムのクラッシュは市場ボラが高い時期に集中する。 — [Hokkaido Univ. WP (2024)](https://eprints.lib.hokudai.ac.jp/repo/huscap/all/97251/FX_May12_2024_submissionR2-SSRN.pdf), [UZH record](https://www.merlin.uzh.ch/publication/show/19856)
- 反対の結果として、Shehadeh-Li-Moore は FX ボラが高い時期のほうがキャリーリターンが正で大きいと報告している（検索結果の要約から引用）。 — [VoxEU column](https://cepr.org/index%2Ephp/voxeu/columns/carry-speculating-volatility-foreign-exchange)
- **2022〜2024年の状況（BIS）**: 2022年初からの急速な円安以降、ヘッジファンドのキャリー感応度はプラスに転じ、2024年7月まで上昇を続けた。2024年8月の巻き戻しでは、日銀の利上げをきっかけに円が急騰し、MXN と BRL が下落、TOPIX は8月5日に二桁の下落となった。BIS は FX の動きを「大きいが過去のキャリークラッシュと比べて桁外れではない」と評価し、市場は約1週間で回復した。BIS は具体的なリターン数値を出していない。 — [BIS Quarterly Review Sept 2024 "Carry off, carry on"](https://www.bis.org/publ/qtrpdf/r_qt2409a.pdf), [BIS Bulletin 90](https://www.bis.org/publ/bisbull90.pdf)
- 2024年の MXN/JPY キャリーは5月まで米国株を上回ったが、メキシコの選挙と日銀のサプライズ利上げを受けて夏までに損失に転じた。 — [Banque de France](https://www.banque-france.fr/en/publications-and-statistics/publications/carry-trades-and-volatility-risk)
- 2026年初のブローカー/リサーチ記事では、G10キャリーは高い金利差と低ボラで復権したとされる。 — [Macrostream (2026)](https://www.macrostream.ai/articles/6a50238bee1fb5bdec94aba7)（ベンダー記事。数値は未確認）

### 推論
- 少数ペアで実装できるキャリーの候補は2つ。
  - (a) ドル・キャリー（EQ-D$ 型）: 米政策金利と G10 の中央値を比べ、対ドル主要通貨バスケット（EURUSD, GBPUSD, AUDUSD, USDJPY など）を一括でロングまたはショートする。月次リバランス。
  - (b) 高金利通貨ロング/低金利通貨（JPY, CHF）ショートを、ボラフィルタ付きで運用する（例: 実現ボラが長期平均を上回ったら縮小/撤退、またはトレンド符号と一致する時だけ保有）。
- 2022〜2025年はドル金利が中央値を上回る期間が長く、EQ-D$ はドルロングだった（推論。公表成績は未確認）。
- **リテールCFDで最大の論点はスワップ**。CFDブローカーのスワップは「金利差 ± ブローカーのマークアップ」で、受け取り側は金利差より小さく、支払い側は大きくなるのが一般的（一般知識で、出典は未確認）。G10 の金利差が1〜3%程度だとマークアップで正味キャリーの相当部分が消えうる。実際の cTrader のスワップ表から、受取/支払の年率換算を通貨ペアごとに算出して確認する必要がある。
- 取引頻度は月次なので、往復1.5bp の手数料は年数bp程度で無視できる。コストの支配項はスワップ。

### 未解決
- 2020〜2026年の G10 キャリー指数（Bloomberg FX Carry, DB G10 Currency Harvest, S&P Risk Premia FX Carry G10）の年次リターンの一次データは取得できなかった。
- ボラフィルタ付きキャリーを2015年以降で OOS 検証した査読論文は見つからなかった。
- リテール FX/CFD のスワップマークアップを定量的に扱った学術研究は見つからなかった。

## Q3. 単一ペアのテクニカル（トレンド）ルール: 時間とともに利益は減ったか

### 要点
文献の結論は一貫していて、先進国通貨の単純な MA/フィルタールールの超過利益は1990年代前半までに消えた。データスヌーピング補正後に残るのは主に EM 通貨で、先進国では JPY など一部に弱い予測力があるのみ。適応的市場仮説（AMH）的な「出現→学習→消滅」のパターンとされる。

### 出典付きの知見
- **Neely-Weller-Ulrich (2009, JFQA 44(2))「The Adaptive Markets Hypothesis: Evidence from the Foreign Exchange Market」**: 1970〜80年代の超過リターンはデータマイニングの産物ではない。ただしフィルタールールと MA ルールの利益機会は1990年代初めまでに消え、あまり研究されていないルールの利益も縮小している（完全に消えたかは不明）。結果は EMH よりも AMH と整合的。 — [IDEAS/JFQA](https://ideas.repec.org/a/cup/jfinqa/v44y2009i02p467-488_09.html), [St. Louis Fed WP 2006-046](https://files.stlouisfed.org/files/htdocs/wp/2006/2006-046.pdf)
- **Olson (2004, JBF)「Have trading rule profits in the currency markets declined over time?」**: 18通貨、1971〜2000年。リスク調整後の MA クロス・オーバーレイ利益は時間とともに低下した。 — [Rohrbach et al. の文献レビュー経由](https://assets.super.so/e46b77e7-ee08-445e-b43f-4ffd88ae0a0e/files/9c27aa78-9b14-4419-a53d-bc56fa9d43b2.pdf)（原典は未取得）
- **Hsu-Taylor-Wang (2016, JIE 102: 188–208)「Technical trading: Is it still beating the foreign exchange market?」**:
  - 対象: 先進国9通貨と EM 21通貨の日次データ、1971年1月〜2015年9月。
  - ルール数: 21,195（フィルター2,835、MA 12,870、サポート/レジスタンス1,890、チャネルブレイクアウト3,000、オシレーター600）。Step-SPA で多重検定を補正。
  - コスト: 片道で先進国0.045%、EM 0.21%。
  - 先進国の予測力は消失している。1980年代に少数のルールが有効だっただけで、1992〜96年のサブ期間以降はゼロ。
  - 全期間で予測可能だったのは先進国9通貨中3通貨（最良は JPY で Sharpe 0.75）、EM 21通貨中10通貨。
  - OOS（2012年1月〜2015年9月、2012年以前の Sharpe で選択）: DEM 0.03、JPY 0.29、NZD 0.54。EM は−0.49〜1.91（平均0.55）。
  - 先進国で有意だった上位ルールはすべて移動平均系だった。
  — [CXO Advisory summary](https://cxoadvisory.com/technical-trading/updated-comprehensive-long-term-test-of-technical-currency-trading), [WUSTL profile](https://profiles.wustl.edu/en/publications/technical-trading-is-it-still-beating-the-foreign-exchange-market/)
- Neely-Weller (2013, St. Louis Fed WP 2011-021)「Lessons from the evolution of foreign exchange trading strategies」: 利益機会は全体としては残るが、学習と競争で徐々に侵食されると述べている。 — [St. Louis Fed WP 2011-021](https://files.stlouisfed.org/files/htdocs/wp/2011/2011-021.pdf)

### 推論
- メジャー通貨ペア（EURUSD, GBPUSD など）単体で、MA クロスやブレイクアウトの単純ルールにエッジを期待するのは文献的に最も分が悪い。グリッドサーチで高 Sharpe が出ても、Step-SPA 級の多重検定補正をかければ消える公算が大きい。USDJPY だけは JPY の予測力が相対的に残る報告があるが、OOS Sharpe は0.29と小さい。
- ユーザーのグリッドサーチ/ウォークフォワード基盤を使うなら、パラメータ数を最小にし（例: 1/3/12か月の符号合成で固定）、ペア横断で同一パラメータにすることがデータスヌーピング対策として重要。

### 未解決
- 2015〜2026年に対象を絞った単一ペア・テクニカルルールの査読付き OOS 研究は見つからなかった。

## Q4. バリュー（PPP）、ドル因子、組み合わせ

### 要点
FXバリュー（実質為替レートの割安・割高）はクロスセクションで予測力を持ち、キャリーやモメンタムとは別の情報を含む（Menkhoff et al. 2017 RFS）。ただし実務指数では2017年以降ほぼフラットで、20〜40通貨のクロスセクションと遅いリバランス（数年単位の平均回帰）を前提とする。少数ペアの個人トレーダーには不向き。組み合わせ（キャリー+モメンタム+バリュー）は分散効果で Sharpe を上げるのが定説だが、主に広いユニバースでの話。

### 出典付きの知見
- **Menkhoff-Sarno-Schmeling-Schrimpf (2017, RFS 30(2): 416–441)「Currency Value」**: 実質為替レートは通貨超過リターンのクロスセクションを予測する。生産性、輸出品の質、対外純資産、産出ギャップで調整した「通貨価値」はより強い予測力を持ち、キャリーやモメンタムとは別の情報を持つ。旧版（2013年）では、実質為替レートの4分位ロングショートが年約4.5%（グロス）、Sharpe 0.51、キャリーとの相関0.55。 — [City Univ. accepted manuscript](https://openaccess.city.ac.uk/id/eprint/14851/13/FXVALUE_Rev3_cepr.pdf), [VoxEU column](https://cepr.org/index%2Ephp/voxeu/columns/currency-valuation-and-risk-premia), [DIW](https://www.diw.de/de/diw_01.c.553899.de/publikationen/externe_referierte_aufsaetze/2017_0002/currency_value.html)
- **実務の状況**: PPP ベースのバリューバスケットは2010年以前は健全な Sharpe だったが、2017年以降ほぼフラット。2005年〜2025年3月の Sharpe は0.2未満で、キャリーとの相関は約0.45。原因は構造的な金利差と資本フローがバリュエーション乖離を上回ったため、とされる。 — [Resonanz Capital](https://resonanzcapital.com/insights/style-premia-are-they-still-relevant)
- 別のブログは、2000〜2024年の様式化された長期 Sharpe をキャリー0.55、モメンタム0.45、バリュー0.35、ドル因子0.30としている。G10キャリーは2008年、2011年、2015年、2024年8月に急落した。 — [Halley substack](https://halleytech.substack.com/p/fx-as-a-factor-zoo-carry-value-momentum-and-dollar)（出典不明確の様式化された数値。参考程度）
- バリューは2008年、2015年、2020年のキャリーのドローダウンを相殺した（ETF運用会社の記事）。同じ記事は、トレンドフォローについて一貫した長期リターンを示していないとも述べている。 — [Simplify](https://www.simplify.us/etfs-use-case/harvesting-foreign-exchange-risk-premia)
- FXモメンタムはリバランス頻度が高く取引コストが大きくなるが、キャリーの下方リスクを打ち消す重要な分散源である（機関投資家の観点のサーベイ）。 — [PMC: Navigating the factor zoo](https://pmc.ncbi.nlm.nih.gov/articles/PMC8074275/)
- ドル因子: 上述の LRV (2014) のドル・キャリーと Daniel-Hodrick-Lu の EQ-D$（Sharpe 0.68、1976〜2013年）。

### 推論
- バリュー単体は個人の少数ペア運用では不適。保有期間が長く（年単位）、2017年以降は機能しておらず、広いクロスセクションが前提。採用するなら「トレンド/キャリーのポジションに対する逆張りの弱いチルト」程度にとどめる。
- 少数ペア×個人で現実的な組み合わせは「TSMOM（マルチアセット: FX主要ペア + XAUUSD + 株価指数）+ ドル・キャリー型の月次ポジション」。両者の相関は低いと考えられる（キャリーはクラッシュ時に負け、トレンドはクラッシュ後の持続局面で勝つ）。文献的な裏付けは分散効果の一般論にとどまる。

### 未解決
- 「Currency Value」の公刊版の正確な Sharpe と、公表後（2017年以降）の学術的な OOS 追試は確認できなかった。

## Q5. ボラティリティ管理/ターゲティング（Moreira-Muir 2017）はFX/金のトレンドを改善するか

### 要点
ボラティリティ・ターゲティング（ポジション = 目標ボラ/実現ボラ）は TSMOM の標準仕様の一部で、リスクを一定にする効果ははっきりしている。一方、Moreira-Muir 型の「ボラ管理で Sharpe/アルファが上がる」という主張は、Cederburg et al. (2020, JFE) が103の株式戦略で OOS では信頼できないと示した。商品モメンタムでも OOS で失敗している。FX キャリーへの適用はクラッシュ回避で一定の支持があるが、OOS 検証は乏しい。

### 出典付きの知見
- Moreira-Muir (2017, JF) のボラ管理ポートフォリオは、高ボラ時にエクスポージャーを縮小する。改善例には通貨キャリーも含まれる。 — [スライド要約](https://gljc.sxu.edu.cn/docs/2022-01/b3783ddbf6e24abb9a4999880496224c.pdf)
- Cederburg-O'Doherty-Wang-Yan (2020, JFE)「On the performance of volatility-managed portfolios」: 103の株式戦略で、in-sample のアルファは正でも、リアルタイム投資家にとってボラ管理版は非管理版を信頼して上回るとは言えない。OOS の確実性等価リターンと Sharpe はむしろ低い。原因はスパニング回帰の不安定性。 — [Lehigh PDF](https://www.lehigh.edu/~xuy219/research/COWY.pdf)
- Kang-Kwon (2021, J. Futures Markets): 商品先物モメンタムでボラ管理は in-sample では有効だが OOS では失敗。 — [IDEAS](https://ideas.repec.org/a/wly/jfutmk/v41y2021i2p159-178.html)
- TSMOM の AQR 実装は、各市場を同一ボラにサイジングし、ポートフォリオを事前ボラ10%に合わせる。 — [AQR PDF](https://www.TrendFollowing.com/whitepaper/Century_Evidence_Trend_Following.pdf)
- FX のキャリー/モメンタムのクラッシュは高ボラ期に集中するため、ボラスケーリングを EM/DM 通貨で検証した研究がある（UZH）。 — [UZH record](https://www.merlin.uzh.ch/publication/show/19856)

### 推論
- 実装上は「各銘柄の名目サイズ = 目標リスク / 実現ボラ（例: 60日 EWMA）」を入れるべき。目的は Sharpe 向上ではなく、ペア間・時間間のリスク均等化と、XAUUSD のような高ボラ銘柄の過大リスクの防止。SL必須の制約とも整合しやすい（SL幅をATR倍数にすれば、1トレードあたりのリスクが自然に一定になる）。
- ボラ上昇時にエクスポージャーを追加で縮小するボラ・タイミングのオーバーレイは、OOS で頑健とは言えないため既定オフが妥当（ユーザー方針「新制約は既定オフ」と整合）。
- 金（XAUUSD）トレンドへのボラ管理の効果を扱った専用研究は見つからなかった。

### 未解決
- FX TSMOM に対するボラ管理を Cederburg 型で OOS 評価した研究は見つからなかった。

## Q6. 日中足（H1/H4）のFXトレンドの証拠

### 要点
FXの日中「モメンタム」研究は存在するが、主に時間帯効果や流動性供給に関する短期の話で、H1/H4 のトレンドフォローが中期戦略として機能するという査読付きの強い証拠は見つからなかった。G10 のモメンタムは主にオーバーナイトで、キャリーは主に日中に稼がれるという分解結果がある。

### 出典付きの知見
- Krohn-Mueller-Whelan「FX Premia Around the Clock」(2018 draft): G10 の高頻度データで、キャリーのリターンは主に日中、モメンタム戦略はオーバーナイトで最も収益的。 — [Krohn-Mueller-Whelan PDF](https://acfr.aut.ac.nz/__data/assets/pdf_file/0007/190753/Krohn_Mueller_Whelan_Jun2018_FXPremiaAroundTheClock.pdf)
- Breedon-Ranaldo: 通貨は自国の取引時間中に減価する傾向があり、EURUSD で単純戦略が成立しうる（2008年草稿）。 — [AEA 2009 paper](https://swlb1.aeaweb.org/conference/2009/retrieve.php?pdfid=301)
- J. Financial Markets 37 (2018)「Intraday momentum in FX markets: Disentangling informed trading from liquidity provision」: タイトルのみ確認し、内容は未確認。 — [IDEAS](https://ideas.repec.org:443/a/eee/finmar/v37y2018icp35-51.html)
- Li-Sakkas-Urquhart (2022, JFM) は株価指数の日中 TSMOM を扱っており、FX ではない。 — [検索結果: Otago archive](https://ourarchive.otago.ac.nz/esploro/outputs/journalArticle/Intraday-time-series-momentum-and-investor-trading/9926495442801891)
- CFM/Bouchaud 系の要約（二次情報で、原典は未確認）: 小ティック契約での高速トレンドの Sharpe は、2009年以前の約0.8から以後はほぼ0に低下した。 — [quantframe 記事](https://www.quantframe.io/article/where-trend-still-works)（信頼度は低い）

### 推論
- H1/H4 のトレンドは、取引回数の増加でコスト（往復1.5bp + スリッページ）の比重が上がる。そのうえ文献上の裏付けも弱い。既に1Hのクロスセクション・リバーサルがコストに負けた経験とも整合する。中期戦略の判断は日足で行い、H1/H4 は執行タイミング（指値/逆指値の置き場所）に限るのが無難。

### 未解決
- FX 主要ペアの H1/H4 トレンドフォローをコスト込みかつ OOS で検証した査読論文は見つからなかった。
