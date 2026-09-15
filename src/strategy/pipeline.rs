use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::chart::{ChartPlotter, ChartPlotterConfig, MultiTimeframeChartData};
use crate::ctrader::CandleBar;
use crate::strategy::features::{find_swing_points, MultiTimeframeBars, PriceActionAnalyzer};
use crate::strategy::types::PriceActionInput;

/// プライスアクション分析とチャート描画の統合結果
#[derive(Debug, Clone)]
pub struct PriceActionBundle {
    /// LLMに入力する構造化特徴量
    pub input: PriceActionInput,
    /// 生成された4分割チャートPNGのファイルパス
    pub chart_path: PathBuf,
}

/// プライスアクション分析 & 画像生成パイプライン
pub struct PriceActionPipeline {
    plotter: ChartPlotter,
    output_dir: PathBuf,
}

impl PriceActionPipeline {
    pub fn new(output_dir: impl AsRef<Path>) -> Self {
        Self {
            plotter: ChartPlotter::with_default(),
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    pub fn with_plotter_config(output_dir: impl AsRef<Path>, config: ChartPlotterConfig) -> Self {
        Self {
            plotter: ChartPlotter::new(config),
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    /// マルチタイムフレームデータから、特徴量抽出と4分割チャート画像の生成を一括実行する
    pub fn process(
        &self,
        pair: &str,
        bars_4h: &[CandleBar],
        bars_1h: &[CandleBar],
        bars_15m: &[CandleBar],
        bars_5m: &[CandleBar],
        spread_pips: f64,
    ) -> Result<PriceActionBundle> {
        // 1. テキスト特徴量（PriceActionInput）の抽出
        let mtf_bars = MultiTimeframeBars {
            pair,
            bars_4h,
            bars_1h,
            bars_15m,
            bars_5m,
            spread_pips,
            current_price: None,
            timestamp: None,
        };
        let input = PriceActionAnalyzer::analyze(&mtf_bars);

        // 2. チャート描画用キーレベル（スイングポイント）の抽出
        let (highs_4h, lows_4h) = find_swing_points(bars_4h, 2);
        let mut key_levels_4h = Vec::new();
        key_levels_4h.extend(highs_4h);
        key_levels_4h.extend(lows_4h);
        key_levels_4h.push(input.market_structure.nearest_h4_resistance);
        key_levels_4h.push(input.market_structure.nearest_h4_support);
        key_levels_4h.sort_by(|a, b| a.partial_cmp(b).unwrap());
        key_levels_4h.dedup_by(|a, b| (*a - *b).abs() < 0.001);

        let (highs_1h, lows_1h) = find_swing_points(bars_1h, 2);
        let mut key_levels_1h = Vec::new();
        key_levels_1h.extend(highs_1h);
        key_levels_1h.extend(lows_1h);

        // 3. チャートファイル名の決定
        let sanitized_time = input.timestamp.replace([' ', ':'], "_");
        let filename = format!("chart_{}_{}.png", pair, sanitized_time);
        let chart_path = self.output_dir.join(filename);

        // 4. チャート画像の生成
        let pattern_detected = &input.five_minute_pa.pattern_detected;
        let chart_data = MultiTimeframeChartData {
            pair,
            bars_4h,
            bars_1h,
            bars_15m,
            bars_5m,
            key_levels_4h,
            key_levels_1h,
            current_price: Some(input.current_price),
            pattern_detected: Some(pattern_detected.as_str()),
        };

        self.plotter.plot_to_file(&chart_data, &chart_path)?;

        Ok(PriceActionBundle { input, chart_path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn make_test_bars(count: usize, base: f64) -> Vec<CandleBar> {
        let now = Utc::now();
        (0..count)
            .map(|i| {
                let p = base + (i as f64 * 0.05);
                CandleBar {
                    timestamp: now + Duration::minutes((i as i64) * 5),
                    open: p,
                    high: p + 0.1,
                    low: p - 0.1,
                    close: p + 0.05,
                    volume: 100,
                }
            })
            .collect()
    }

    #[test]
    fn test_pipeline_process() {
        let temp_dir = std::env::temp_dir().join(format!("ntrade_test_pipeline_{}", std::process::id()));
        let pipeline = PriceActionPipeline::new(&temp_dir);

        let bars_4h = make_test_bars(25, 154.0);
        let bars_1h = make_test_bars(30, 154.2);
        let bars_15m = make_test_bars(35, 154.4);
        let bars_5m = make_test_bars(40, 154.5);

        let result = pipeline.process("USDJPY", &bars_4h, &bars_1h, &bars_15m, &bars_5m, 0.2);
        assert!(result.is_ok(), "Pipeline process failed: {:?}", result.err());

        let bundle = result.unwrap();
        assert_eq!(bundle.input.pair, "USDJPY");
        assert!(bundle.chart_path.exists());
        assert!(bundle.chart_path.extension().unwrap() == "png");

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
