# FX の日中・カレンダー・フロー起因アノマリー（バーベースのリテール執行で使えるか）

前提: 往復コスト約 1.5 bp（USDJPY で 1.5–2 pips、スリッページ込み）。cTrader ゼロスプレッド＋手数料、M1–D1 バー、バー確定で判断、SL 必須。棄却済みのため除外したもの: ゴトー日／東京仲値の USDJPY 売り、指数→クロス円の1分リードラグ、8通貨の強弱モメンタム、AUDNZD/EURGBP の z スコア平均回帰。
換算目安: 年率 5% ÷ 252 営業日 ≈ 1 日 2 bp。1 bp は USDJPY 150 円で約 1.5 pips、EURUSD 1.10 で約 1.1 pips。

## Q1. 月末・四半期末のリバランスフローと WM/R ロンドン 4pm フィックス（Melvin-Prins 2015、Evans 2018、Ito-Yamada 2018）

### Takeaway
月末にはフィックス前1時間で、当月の相対株価パフォーマンスとは逆向きに通貨が動く傾向が学術的に確認されている。ただし効果は株価の相対差10%あたり14 bp で、R² は0.03。サンプルは2015年の改革以前（2004–2012年）で、2015年以降の独立した検証は見つからなかった。月によっては相対差が大きくなるため、条件付きなら 1.5 bp のコストを超える可能性はある。とはいえ、公表後も効果が残っているかは確認できていない。

### Cited Findings
- Melvin & Prins は、月末最終営業日の 15:00–16:00（ロンドン時間）の通貨リターンを、その国の株価指数の月初から最終前日までのリターン（クロスセクションで平均を引いたもの）に回帰した。パネル係数は −0.0142（p=0.00）、R²=0.03、F=25.52。株価が他国より10%上回ると、フィックス前1時間でその国の通貨が約14 bp 下落する。サンプルは2004-04-28〜2012-12-31、対象は USD, EUR, JPY, GBP, CAD, AUD, SEK, NOK, CHF, NZD の10通貨 — [Melvin & Prins ECB workshop draft 2013](https://www.ecb.europa.eu/events/pdf/conferences/131216/Third_FX_Workshop_MELVIN_PRINS_Equity%20hedging%20and%20exchange%20rates%20Nov%202013.pdf)；掲載版は [J. Financial Markets 22 (2015) 50–72](https://ideas.repec.org/a/eee/finmar/v22y2015icp50-72.html)
- 仕組み: ヘッジャーは約1時間前に銀行へ注文を出し、フィックスレートで約定を受ける。ヘッジの調整は通常、毎日ではなく月末のフィックスでまとめて行う — [Melvin & Prins draft](https://www.ecb.europa.eu/events/pdf/conferences/131216/Third_FX_Workshop_MELVIN_PRINS_Equity%20hedging%20and%20exchange%20rates%20Nov%202013.pdf)
- 反転: 「16:00〜翌日12:00 のリターン」を「15:00〜16:00 のリターン」と、それに月末ダミーを掛けた項に回帰すると、係数は −0.339 と −0.387（どちらも p=0.00、R²=1.9%）。月末日にはフィックス前1時間の値動きの約72%が翌日正午までに戻る。月末日の反転は通常日の約2倍 — [Melvin & Prins draft](https://www.ecb.europa.eu/events/pdf/conferences/131216/Third_FX_Workshop_MELVIN_PRINS_Equity%20hedging%20and%20exchange%20rates%20Nov%202013.pdf)
- Evans: 21通貨・約10年のデータで、フィックス前後の値動きは異常に大きなボラティリティと強い負の系列相関を示し、特に月末最終営業日で顕著だった — [Evans MPRA 81583 (JBF forthcoming)](https://mpra.ub.uni-muenchen.de/81583)；[2014 draft](https://mpra.ub.uni-muenchen.de/60171)
- Ito & Yamada（JIMF 2018）: 2015年の改革でフィックス窓は5分に延び、窓内の出来高スパイクは消えた。それでも従来指摘されていた価格の異常は改革後も残り、銀行のパッシブな執行が新たな予測可能性を生んでいる。数値は要旨に載っていない — [Ito & Yamada, JIMF 80 (2018)](https://ideas.repec.org/a/eee/jimfin/v80y2018icp75-95.html)
- 実務家の見方（2026年7月）: 月末のフィックスフローは今も問題で、5分の窓では量をさばききれない。一方で、一部の参加者が別の日や時間に執行を移したため、月末フローはやや減ったとされる — [The Full FX "The Last Look"](https://thefullfx.com/the-last-look-259/)
- 実務家の見方: セルサイドが月末リバランスの推計を数日前から公表しており、ポジションが早めに取られている可能性がある（研究ではなく検討用のメモ） — [GitHub DataScienceAndTrading issue #27](https://github.com/DougOscar/DataScienceAndTrading/issues/27)；仕組みの解説（米株が他国より強い月はドル売り、弱い月はドル買い） — [piphawk guide](https://piphawk.com/guides/month-end-rebalancing-fx-flows)

### Inferences
- 実装案: 月末最終営業日に、各通貨国の株価指数 CFD の月初から最終前日までのリターンを算出し、平均を引く。最も強い国の通貨を売り、最も弱い国の通貨を買う（例: 米株が欧州株を4%上回ったら EURUSD を買う＝ドル売り）。ロンドン 15:00 の M5/M15 バー確定で成行エントリーし、16:05 頃に決済。SL は過去の同時間帯 ATR の 1.5–2 倍。期待値は「相対差 × 1.4 bp/%」。相対差 4% なら約 5.6 bp でコストの約3.7倍、相対差 1% なら 1.4 bp でコスト未満。そのため「|相対差| > 3%」のような閾値フィルタが要る。
- 反転トレード: 16:00 に前1時間の方向と逆にエントリーし、翌日ロンドン 12:00 に決済する。前1時間の動きが 10 bp なら約 7 bp の戻りが期待できる計算になる。ただし保有時間が長く、ノイズも大きい。
- 試行は年12回しかないため、統計的な検証力は低い。2015年以降のデータ（約130回）でも、効果がコストを超えるかの判定は難しい。四半期末と年末はフローが大きいが、回数はさらに少ない。
- 月末 15:00–16:00 のロンドン時間はスプレッドが広がりやすい。ゼロスプレッド口座でも実際の約定コストは平常時より大きいと想定すべき（推測）。

### Gaps
- 2015年以降（特に2018–2026年）に Melvin-Prins の回帰を再推定した研究は見つからなかった。
- Ito-Yamada の改革後の数値は全文が有料で入手できなかった。
- フィックス窓が5分に延びた後、ヘッジ執行が TWAP 化したり前倒しされたりしてシグナルのタイミングがずれたかは不明。

## Q2. 日中の季節性と「W字型」パターン（Ranaldo 2009、Breedon-Ranaldo 2013、Krohn-Mueller-Whelan 2024）

### Takeaway
「現地時間帯にその国の通貨が売られる」パターンと、フィックス前にドル高・フィックス後にドル安となる「W字型」パターンは、1999–2019年に t 値 4.5–11.7 で頑健に存在した。1区間あたりの大きさは約 2–2.5 bp/日で、往復 1.5 bp のコストとほぼ同じ水準しかない。しかも2018–2019年には縮小しており、著者自身も「平均的なトレーダーには利用は難しい」と結論している。

### Cited Findings
- KMW（JF 2024 / BoC SWP 2021-48）: G9通貨（AUD, CAD, EUR, GBP, JPY, NZD, NOK, SEK, CHF）を等ウェイトにしたドルポートフォリオ（外貨買い）の動き。NY クローズから東京フィックスまでに年率 −5.0%、その後に +5.3%。欧州朝から ECB フィックスまでに −4.2%、ロンドンフィックスから NY クローズまでに +4.6%。t 値は 4.5–11.7。1日あたりではフィックス前に約 2.5 bp のドル高、その後に同程度のドル安。期間は1999-01〜2019-12、5分足を使用 — [KMW BoC SWP 2021-48](https://www.bankofcanada.ca/wp-content/uploads/2021/10/swp2021-48.pdf)
- 区間の定義（ET）: pre-Tokyo は 17:00–20:55（冬時間は 19:55 で、東京 9:55 に合わせる）、post-Tokyo は 20:55–02:00、pre-ECB は 02:00–08:15、ECB→London は 08:15–11:00、post-London は 11:00–17:00 — [KMW](https://www.bankofcanada.ca/wp-content/uploads/2021/10/swp2021-48.pdf)
- 通貨別: 東京フィックス前の下落が最も大きいのは AUD（年率 −7.15%）と NZD（−8.53%）。ECB フィックス前は EUR（−8.87%）と SEK（−7.73%）。ロンドンフィックス後はほとんどの通貨が 3.89%（CAD）〜6.87%（EUR）上昇するが、JPY だけは −2.92% と逆に下落する — [KMW](https://www.bankofcanada.ca/wp-content/uploads/2021/10/swp2021-48.pdf)
- 時系列の安定性: 1999–2014年は各側で年率約 6.5% だった。2014–2019年は東京フィックス周辺の反転が明確に小さくなった（ただし有意性は残る）。東京は2013年以降 5% 以下で、2018年に初めてマイナスの年が出た。欧州フィックス周辺は金融危機後も年率約 10% を保ったが、サンプル最後の2年（2018–2019年）で 5% 未満に落ちた。著者は、裁定資金の流入と低ボラティリティ期に効果が弱まることを指摘している — [KMW](https://www.bankofcanada.ca/wp-content/uploads/2021/10/swp2021-48.pdf)
- コスト込みの結果: 指標的なビッド・アスクを全額払うと、ほとんどの区間でリターンがマイナスになる。スプレッドを50%に減らすと、東京戦略は EUR 2.65%、GBP 0.51%、JPY 2.73%。欧州戦略（ECB 前にドル買い、ロンドン後にドル売り）は ECB 前 8.64%、ロンドン後 5.46%（いずれも年率）。シャープレシオは 0.12–1.03 で、著者は「平均的なトレーダーが利用できるかは明らかでない」と述べている — [KMW](https://www.bankofcanada.ca/wp-content/uploads/2021/10/swp2021-48.pdf)
- KMW: 反転の原因はフィックスそのものではなく、ディーラーのヘッジだと結論。BoC 版の要旨では、パターンは曜日・月・20年すべてに見られる — [Warwick repository](https://wrap.warwick.ac.uk/id/eprint/177333)；[BoC page](https://www.bankofcanada.ca/2021/10/staff-working-paper-2021-48/)
- Ranaldo（SNB WP 2007-03）: 自国通貨は自国の取引時間に下落し、他国の取引時間に上昇する。この傾向は統計的にも経済的にも高度に有意で、カレンダー効果を除いても長年続いている。原因はホームバイアスと市場の分断によるディーラー在庫の偏りと説明されている — [SNB WP 2007-03](https://www.snb.ch/en/publications/research/working-papers/2007/working_paper_2007_03)
- Breedon-Ranaldo: EBS の取引データ 1997-01〜2007-06。執行ルールは EUR/USD を GMT 7:00–13:00 にショート、13:00–21:00 にロング。EBS の実際に約定可能な bid/ask でコストを差し引いた後でも、年率 6%（シャープ 1.3）と 7%（シャープ 0.9）。USD/JPY、EUR/JPY、GBP/USD、USD/CHF、AUD/USD の同種ルールはコスト込みでマイナス（年率 −2%〜−51%）。EUR/USD の欧州時間と米国時間のリターン差は年ごとに安定していた — [Breedon & Ranaldo 2008 draft (AEA 2009)](https://swlb1.aeaweb.org/conference/2009/retrieve.php?pdfid=301)；[SNB WP 2011-04](https://www.snb.ch/en/publications/research/working-papers/2011/working_paper_2011_04)
- 2020–2024年に別の著者が独立に追試した研究は見つからなかった。実務家ブログは EURUSD の M15 で各フィックスの前後15分を売買するルールを公開しているが、結果は有料部分にあり確認できない — [defitrading substack](https://defitrading.substack.com/p/profiting-from-fx-fixes)

### Inferences
- 実装案A（欧州W字、EURUSD/GBPUSD）: ロンドン 07:00 にドルを買う（EURUSD 売り）→ 13:15 の ECB フィックスで決済。ロンドン 16:00 にドルを売る（EURUSD 買い）→ NY 17:00 で決済。各区間の総期待値は年率 5–9% で、1日あたり約 2–3.5 bp。往復 1.5 bp を引くと 0.5–2 bp しか残らない。しかも2018–2019年に縮小したため、2020年以降の自前バックテストが必須。JPY はロンドン後の符号が逆なので除外する。
- 実装案B（Breedon-Ranaldo 型）: GMT 7–13 に EURUSD をショート、13–21 にロング。ただし実質的に案Aとほぼ同じ現象を見ている。
- 1日の総期待値 2–3 bp に対してコストは 1.5 bp で、取引回数が非常に多い（毎日1–2往復）。わずかなコストの悪化で優位性が消えるため、優先度は中から低。M15 で検証しやすいので、2019–2026年のデータで「総期待値が 3 bp を超える区間があるか」を確認する価値はある。
- 指値を使ってスプレッドを取る側に回れれば改善するが、ゼロスプレッド＋手数料体系では指値の利点は小さい（推測）。

### Gaps
- KMW のサンプルは2019年で終わっている。2020–2026年の W 字の大きさを示す一次資料は見つからなかった。
- Ranaldo 2009 の大きさ（bp）は全文を確認できていない。

## Q3. FOMC・マクロ指標発表の前後ドリフト、日中モメンタム

### Takeaway
FOMC 発表日にはドルショート（キャリー寄り）のバスケットの超過リターンが大きいことが知られている（Mueller-Tahbaz-Salehi-Vedolin、JF 2017）。また、FF 金利先物が示す方向に発表前2日間ドルが動く現象（Karnaukh）は1回あたり約 25 bp と、コストに対して桁違いに大きい。一方で、高頻度データでは発表前ドリフトの約65%が発表後 12–24 時間で戻る（Lee-Wang 2025）。NFP などの指標後のドリフトは数分〜数時間で消え、バーベースのモメンタムとしての根拠は弱い。

### Cited Findings
- Mueller, Tahbaz-Salehi & Vedolin（JF 72(3) 2017, pp.1213–1252）: ドルを売って外貨を買う戦略は、FOMC 発表予定日に有意に大きな超過リターンを生む。超過リターンは米国との金利差が大きい通貨ほど大きく、政策の不確実性が高いほど、さらに FRB が緩和方向にあるほど大きくなる。著者はこれを金融政策の不確実性へのプレミアムと解釈している — [LSE Research Online](https://researchonline.lse.ac.uk/id/eprint/77256)；[FMG DP-54](https://www.fmg.ac.uk/sites/default/files/publications/dp-54.pdf)
- Karnaukh（"The Dollar Ahead of FOMC Target Rate Changes"）: 利上げを発表する FOMC の2日前からドルはバスケットに対して上昇し、利下げ前には下落する。FF 金利先物が FX より先に動き、それを使ったドルのロング/ショート戦略は平均超過リターン 25 bp、シャープ 0.93（会議版では t 値 2.8）。サンプル期間と詳細は PDF にアクセスできず（403）確認していない — [Karnaukh paper (AUT ACFR)](https://acfr.aut.ac.nz/__data/assets/pdf_file/0019/190315/DollarAheadFOMC_NinaKarnaukh.pdf)；[AEA 2017 version](https://topcat.aeaweb.org/conference/2017/preliminary/paper/RAK7GsSa)
- Lee & Wang（RAPS 15(3-4), 2025）: 高頻度データでは FOMC 発表後のリターンが有意に低く、発表前の正のドリフトの約65%を打ち消す。反転の多くは発表から 12–24 時間後に起き、ジャンプの大きさや流動性の低さと関係している — [IDEAS: Lee & Wang 2025](https://ideas.repec.org/a/oup/rasset/v15y2025i3-4p247-287..html)
- 反証: Lucca & Moench（2012）は、株式で見られる発表前ドリフトがドルの為替レートでは検出されないと報告した（検索結果の要約による。一次資料は未確認） — [search summary referencing Lucca-Moench; see Neuhierl-Weber](https://www.lawfin.uni-frankfurt.de/fileadmin/user_upload/ANeuhierl_paper_09252020.pdf)
- 指標発表への反応: DEM/USD では米指標への反応は発表後15分に集中し、3時間以内にほぼ消える（Almeida-Goodhart-Payne 1997、二次資料経由） — [LSE: effects of macroeconomic news on high-frequency exchange rate behaviour](https://researchonline.lse.ac.uk/id/eprint/119153)
- NFP 237回（2000–2020年）のイベントスタディ（修士論文）: 異常リターンは有意だが、2017–2020年は反応が小さくなった — [Jandejsek MBA thesis 2021](https://web-typo3.modul.ac.at/uploads/files/Theses/Master/Grad_2021/MBA_2021/Master_Thesis_Jandejsek.pdf)
- 日中モメンタム（最初の30分が最後の30分を予測する）: 根拠の中心は株式（Gao-Han-Li-Zhou、SPY 1993–2013年）。FX で引用されるのは RUB-USD くらいで、主要通貨ペアで確立された証拠は見つからなかった — [Gao et al. PDF](https://c.mql5.com/forextsd/forum/173/intraday_momentum_-_the_first_half-hour_return_predicts_the_last_half-hour_return.pdf)；[IDEAS PBFJ 2023 (Asia-Pacific)](https://ideas.repec.org/a/eee/pacfin/v80y2023ics0927538x2300152x.html)

### Inferences
- 実装案（FOMC 前のドル方向）: FOMC の2営業日前の NY 17:00 時点で、FF 金利先物（または OIS）が示す変更確率を確認する。利上げ確率が高ければドルを買い（EURUSD/AUDUSD 売り）、利下げ確率が高ければドルを売る。発表直前（ET 13:55 頃）に決済し、発表のジャンプを避ける。期待値 25 bp に対してコスト 1.5 bp で比率は大きい。ただし年8回しかなく、金利の織り込みデータを外部から取得する必要がある。現在の Rust 戦略の枠組みにはない外部データ依存になる。
- 実装案（FOMC 日のドル売りキャリー）: FOMC 発表日に、米国との金利差が大きい通貨（AUD, NZD など）を対ドルで買う。NY の前日 17:00 から当日 17:00 まで保有。大きさは全文で確認していないため、自前で検証する必要がある。発表後 12–24 時間の反転（Lee-Wang）を考えると、発表直後に決済するのが合理的。
- NFP/CPI 後のモメンタム追随は、バー確定時点で既に反応の大半が終わっているため、根拠は弱い。

### Gaps
- MTV 2017 の具体的な bp 値（FOMC 日の超過リターン）と、2017年以降に再検証されたかは未確認。
- Karnaukh のサンプル期間と、ゼロ金利期やフォワードガイダンス期の扱いは未確認。
- 主要通貨ペアで、指標発表後の数時間〜1日のドリフトを示す査読論文は見つからなかった。

## Q4. 週末ギャップ・月曜効果、アジアレンジのロンドンオープンブレイク（ORB）

### Takeaway
どちらにも査読された学術的根拠は見つからなかった。実務家の検証では、EURUSD のロンドンブレイクアウトは 2016–2026年の H1 でほぼ損益ゼロ。週末ギャップの多くは 10–15 pips 未満と小さく、取引する価値が乏しい。XAUUSD のロンドンブレイクアウトで PF 1.38–1.61 という自己申告の結果が1件あるが、サンプルが小さく信頼性は低い。

### Cited Findings
- ロンドンブレイクアウト（EURUSD、2016–2026年、H1）: 勝率 36.3% で、損益分岐（約33%）をわずかに上回る程度。実際の損益を見るとその差も消える — [backtrex London breakout EURUSD](https://backtrex.com/en/backtests/london-breakout-eur-usd)
- XAUUSD のロンドンブレイクアウト（2023-01〜2026-09）: PF 約 1.38、DD 17–19%。2026年のアウトオブサンプルでは PF 1.61（157取引）。パラメータは 2023–2025年で選んだと作者が述べており、自己申告かつ少サンプル — [MQL5 blog 777540](https://www.mql5.com/en/blogs/post/777540)
- 欧米の夏時間の切り替え日がずれるため、「ロンドンオープン」はサーバー時刻で年中同じではない。時間帯別に分けて検証するよう警告している — [MQL5 blog 774527](https://www.mql5.com/en/blogs/post/774527)
- 週末ギャップ: 前日終値への戻りに普遍的な傾向はない（大きいギャップは短期ボラティリティを伴う） — [Harbourfront Quant substack](https://harbourfrontquant.substack.com/p/are-weekend-gaps-always-filled-a)
- EURUSD のギャップは6か月で約80%埋まるが、ほとんどが 10 pips 未満で取引の価値は乏しい — [TradeThatSwing](https://tradethatswing.com/eurusd-monday-gap-fill-strategy-and-statistics/)
- EURUSD・GBPUSD・USDJPY の日曜オープン500回では、約68%が 15 pips 未満。オープン時のスプレッド拡大がギャップ逆張りの利益を削る（ベンダーのブログ） — [FXNX](https://fxnx.com/en/blog/forex-weekend-gap-size-500-sunday-opens-their-fill-rates)

### Inferences
- 週末ギャップの逆張りは、期待値の上限（ギャップの幅）が小さい。週明けオープン直後はスプレッドが広く、流動性も低い。コストを考えると優先度は低い。
- アジアレンジのブレイクアウトは、KMW の「ECB フィックス前（欧州朝）にドル高」という傾向と組み合わせるなら意味があるかもしれない。たとえばロンドン 07:00 のブレイクのうちドル高方向だけを取る。ただしこれは推測で、根拠はない。
- XAUUSD のロンドンブレイクは唯一プラスの報告だが、ユーザー自身の検証基盤で 2019–2026年の OOS を確認するまでは「未検証」として扱うべき。

### Gaps
- FX のオープニングレンジブレイクアウトや週末効果に関する、2015年以降の査読論文は見つからなかった。

## Q5. 日本固有の効果（BoJ、日本の年度末など）。ゴトー日は棄却済み

### Takeaway
「3月末に国内企業が海外利益を本国へ戻すので円高になる」という通説は、検証すると根拠が弱い。2000–2011年の ANZ の分析では3月の USDJPY は6勝6敗で、3月中旬〜4月初めはむしろ10年中9年で USDJPY が上昇した（円安）。仲値のゴトー日以外で、内容が大きく異なる新しい証拠は見つからなかった。ただ、KMW では東京フィックス前に JPY が下落し、ロンドンフィックス後に JPY だけ他通貨と逆に下落するという通貨別の特徴が確認されている。

### Cited Findings
- ANZ（2012年、FNArena 経由）: 2000年以降、3月の USDJPY は6年上昇・6年下落。3月中旬〜4月初めは2004年を除く10年で上昇しており、レパトリによる円高は「現実より神話」 — [FNArena](https://fnarena.com/?p=59599)
- 3月最終週に円高になるという季節性の主張（2023年の単発の市場解説） — [FOREX.com](https://forex.com/en/news-and-analysis/japanese-yen-analysis-usdjpy-breaks-out-despite-bearish-seasonality)
- KMW の通貨別結果: 東京フィックス後の反転は JPY で年率 7.94% と最大。ロンドンフィックス後は JPY だけが −2.92% 下落し、他通貨と逆。欧州フィックス周辺で JPY を売買すると、1999–2019年で約6%の損失 — [KMW](https://www.bankofcanada.ca/wp-content/uploads/2021/10/swp2021-48.pdf)

### Inferences
- 東京フィックス周辺の JPY 反転（東京 9:55 までドル高、その後ドル安）は、ユーザーが棄却したゴトー日 USDJPY 売りと同じ現象を見ている。KMW でも東京の効果は2018年に初めてマイナスの年が出ており、ユーザーの 2026年 PF 0.71 という結果と整合的。再検証の優先度は低い。
- 年度末の円レパトリ効果は、長期間の検証で支持されていない。

### Gaps
- BoJ 会合日の前後に JPY のドリフトがあるかを示す学術研究は、この調査では見つからなかった（未検索の部分あり）。

## Q6. 祝日効果、年末のドル資金調達

### Takeaway
年末（特に12月中旬〜1月第1週）のドル資金調達の逼迫は、FX スワップやベーシスでは明確に確認されている。しかし、スポットのドル方向を予測できるという根拠は見つからなかった。月末のドル方向や turn-of-month 効果も、主要通貨ペアについての最近の証拠はない。

### Cited Findings
- CME: 年末にはドルの FX 資金調達レートが上がる。逼迫は年末の約1週間前に最も強く、12月中旬のピークから1月第1週末までに調達コストの差は平均 −107 bp 縮小した。他の四半期末では同様の上昇は見られない — [CME Group 2022 year-end FX turn](https://cmegroup.com/articles/2022/2022-year-end-fx-turn.html)；[CME year-end effects in FX swap](https://www.cmegroup.com/education/articles-and-reports/understanding-and-analyzing-year-end-effects-in-the-fx-swap-market.html)
- 年末の数週間は FX のボラティリティが大きく下がる（2014年の市場解説） — [FXStreet 2014-12-19](https://www.fxstreet.com/analysis/fundamental-forex-commentary/2014/12/19)
- Turn-of-month: インドの通貨市場（1999–2014年）では USD/INR と JPY/INR で月替わり日のリターンが有意に低く、2008年以降は USD 以外で消えた。主要通貨ペアの話ではない — [Kumar SSRN 2625939](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2625939)
- 月末の流動性需要についての研究「Dash for Cash」（AEA 2016）は存在するが、為替への具体的な結論は確認していない — [AEA 2016 Dash for Cash](https://www.aeaweb.org/conference/2016/retrieve.php?pdfid=21226&tk=zHQNB4Rz)

### Inferences
- 年末のドル資金調達の逼迫はスワップポイント（ロールオーバーコスト）に影響するため、CFD で年末をまたいで保有する場合のコスト要因として扱うのが妥当。スポット方向の戦略としての根拠はない。

### Gaps
- 主要通貨ペアのスポットについて、祝日（米国やロンドンが休場で他市場が開いている日）の方向性を検証した最近の研究は見つからなかった。

---
### 総括（コストとの比較。各数値の出典は上の各セクション）
| 効果 | 1回あたりの総期待値 | 試行頻度 | 公表後（2018年以降）の状況 | コスト 1.5 bp に対する評価 |
|---|---|---|---|---|
| 月末フィックス前1時間（相対株価に逆張り） | 株価相対差10%あたり 14 bp（相対差4%なら約 5.6 bp） | 年12回 | 未検証（サンプルは 2004–2012年） | 条件付きなら有望、ただし検証力が低い |
| 月末フィックス後の反転（16:00→翌日12:00） | 前1時間の値動きの約72% | 年12回 | 未検証 | 前1時間の動きが大きい月に限れば候補 |
| W字（ECB前ドル買い／ロンドン後ドル売り） | 1区間あたり約 2–3.5 bp | 毎日 | 2018–2019年に縮小 | コストとほぼ同じ、優位性は薄い |
| Breedon-Ranaldo EURUSD | 年率 6–7%（EBS のコスト込み） | 毎日 | 確認できず | 薄い |
| FOMC 前2日のドル方向（FF先物に従う） | 約 25 bp | 年8回 | 未確認 | コスト比では最大。外部データが必要 |
| FOMC 日のドル売り（金利差の大きい通貨を買う） | 数値は未確認 | 年8回 | 未確認 | 要検証 |
| ロンドンブレイク・週末ギャップ | 学術的根拠なし | — | — | 低い |
| 日本の年度末・東京仲値 | 支持されない／棄却済み | — | — | 除外 |
