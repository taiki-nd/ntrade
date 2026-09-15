use anyhow::{Context, Result};
use plotters::prelude::*;
use std::fs;
use std::path::Path;

use crate::ctrader::CandleBar;
use crate::strategy::features::{calculate_ema, get_pip_size};

/// チャート生成設定
#[derive(Debug, Clone)]
pub struct ChartPlotterConfig {
    pub width: u32,
    pub height: u32,
    pub bars_to_display: usize,
    pub dark_mode: bool,
}

impl Default for ChartPlotterConfig {
    fn default() -> Self {
        Self {
            width: 1600,
            height: 1200,
            bars_to_display: 45,
            dark_mode: true,
        }
    }
}

/// マルチタイムフレームチャート描画用データ
pub struct MultiTimeframeChartData<'a> {
    pub pair: &'a str,
    pub bars_4h: &'a [CandleBar],
    pub bars_1h: &'a [CandleBar],
    pub bars_15m: &'a [CandleBar],
    pub bars_5m: &'a [CandleBar],
    pub key_levels_4h: Vec<f64>,
    pub key_levels_1h: Vec<f64>,
    pub current_price: Option<f64>,
    pub pattern_detected: Option<&'a str>,
}

/// 4分割マルチタイムフレームチャート画像ジェネレータ
pub struct ChartPlotter {
    config: ChartPlotterConfig,
}

impl ChartPlotter {
    pub fn new(config: ChartPlotterConfig) -> Self {
        Self { config }
    }

    /// デフォルト設定でインスタンスを作成
    pub fn with_default() -> Self {
        Self::new(ChartPlotterConfig::default())
    }

    /// チャート画像をPNGファイルとして保存する
    pub fn plot_to_file(&self, data: &MultiTimeframeChartData, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create chart directory: {:?}", parent))?;
        }

        let root = BitMapBackend::new(path, (self.config.width, self.config.height))
            .into_drawing_area();

        self.render_all(&root, data)?;

        root.present()
            .with_context(|| format!("Failed to present and save chart image to {:?}", path))?;

        Ok(())
    }

    /// チャート画像をPNGバイト列として生成する
    pub fn plot_to_bytes(&self, data: &MultiTimeframeChartData) -> Result<Vec<u8>> {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("ntrade_chart_{}.png", std::process::id()));

        self.plot_to_file(data, &temp_path)?;
        let bytes = fs::read(&temp_path)
            .with_context(|| format!("Failed to read generated chart from {:?}", temp_path))?;
        let _ = fs::remove_file(&temp_path);

        Ok(bytes)
    }

    /// 4分割エリア全体の描画
    fn render_all<DB: DrawingBackend>(
        &self,
        root: &DrawingArea<DB, plotters::coord::Shift>,
        data: &MultiTimeframeChartData,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        // 背景の塗りつぶし (TradingView風 ダークテーマ: #131722)
        let bg_color = RGBColor(19, 23, 34);
        root.fill(&bg_color)?;

        // 2x2 に等分割
        let sub_areas = root.split_evenly((2, 2));

        // 1. 左上: 4H
        let title_4h = format!("{}  [ 4H - MACRO CONTEXT ]", data.pair);
        self.render_panel(
            &sub_areas[0],
            data.pair,
            &title_4h,
            data.bars_4h,
            &data.key_levels_4h,
            data.current_price,
            false,
            None,
        )?;

        // 2. 右上: 1H
        let title_1h = format!("{}  [ 1H - SWING STRUCTURE & EMA ]", data.pair);
        self.render_panel(
            &sub_areas[1],
            data.pair,
            &title_1h,
            data.bars_1h,
            &data.key_levels_1h,
            data.current_price,
            false,
            None,
        )?;

        // 3. 左下: 15M
        let title_15m = format!("{}  [ 15M - SETUP & PULLBACK ]", data.pair);
        self.render_panel(
            &sub_areas[2],
            data.pair,
            &title_15m,
            data.bars_15m,
            &[],
            data.current_price,
            false,
            None,
        )?;

        // 4. 右下: 5M (トリガー足 & フォーカス)
        let pattern_str = data.pattern_detected.unwrap_or("NONE");
        let title_5m = format!(
            "{}  [ 5M - ENTRY TRIGGER ]  >> Trigger: {}",
            data.pair, pattern_str
        );
        self.render_panel(
            &sub_areas[3],
            data.pair,
            &title_5m,
            data.bars_5m,
            &[],
            data.current_price,
            true,
            data.pattern_detected,
        )?;

        Ok(())
    }

    /// 各時間足サブパネルの描画
    #[allow(clippy::too_many_arguments)]
    fn render_panel<DB: DrawingBackend>(
        &self,
        area: &DrawingArea<DB, plotters::coord::Shift>,
        pair: &str,
        title: &str,
        all_bars: &[CandleBar],
        key_levels: &[f64],
        current_price: Option<f64>,
        is_trigger_panel: bool,
        pattern_detected: Option<&str>,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        let pip_size = get_pip_size(pair);
        let digits = if pip_size >= 0.01 { 3 } else { 5 };

        // 直近 N 本に制限
        let display_count = self.config.bars_to_display;
        let start_idx = all_bars.len().saturating_sub(display_count);
        let display_bars = &all_bars[start_idx..];

        if display_bars.is_empty() {
            return Ok(());
        }

        // EMAの計算（全体データから計算し、表示区間のみ切り出す）
        let ema20_all = calculate_ema(all_bars, 20);
        let ema50_all = calculate_ema(all_bars, 50);
        let ema20_slice = &ema20_all[start_idx..];
        let ema50_slice = &ema50_all[start_idx..];

        // 価格範囲（min / max）を算出
        let mut min_p = display_bars
            .iter()
            .map(|b| b.low)
            .fold(f64::INFINITY, f64::min);
        let mut max_p = display_bars
            .iter()
            .map(|b| b.high)
            .fold(f64::NEG_INFINITY, f64::max);

        // EMA値も価格範囲に含める
        for &e in ema20_slice.iter().flatten() {
            min_p = min_p.min(e);
            max_p = max_p.max(e);
        }
        for &e in ema50_slice.iter().flatten() {
            min_p = min_p.min(e);
            max_p = max_p.max(e);
        }

        // マージンを付加（上下 8%）
        let range = (max_p - min_p).max(pip_size * 5.0);
        let y_min = min_p - range * 0.08;
        let y_max = max_p + range * 0.08;

        let num_bars = display_bars.len();

        // チャートビルダーの初期化
        let mut chart = ChartBuilder::on(area)
            .caption(
                title,
                ("sans-serif", 15)
                    .into_font()
                    .color(&RGBColor(240, 242, 245)),
            )
            .margin(6)
            .x_label_area_size(24)
            .y_label_area_size(70)
            .build_cartesian_2d(0..num_bars, y_min..y_max)?;

        // メッシュと軸のスタイリング (TradingView Dark配色)
        let axis_color = RGBColor(70, 77, 95);
        let grid_bold = RGBColor(35, 41, 55);
        let grid_light = RGBColor(25, 30, 42);
        let label_color = RGBColor(160, 168, 185);

        chart
            .configure_mesh()
            .axis_style(axis_color)
            .bold_line_style(grid_bold)
            .light_line_style(grid_light)
            .label_style(("sans-serif", 11).into_font().color(&label_color))
            .x_labels(8)
            .y_labels(8)
            .x_label_formatter(&|&x| {
                if x < display_bars.len() {
                    display_bars[x].timestamp.format("%H:%M").to_string()
                } else {
                    "".to_string()
                }
            })
            .y_label_formatter(&move |&y| {
                if digits == 3 {
                    format!("{:.3}", y)
                } else {
                    format!("{:.5}", y)
                }
            })
            .draw()?;

        // 1. 水平サポレジラインの描画
        let level_color = RGBColor(186, 104, 200); // パープル
        for &lvl in key_levels {
            if lvl >= y_min && lvl <= y_max {
                chart.draw_series(LineSeries::new(
                    vec![(0, lvl), (num_bars, lvl)],
                    ShapeStyle {
                        color: RGBAColor(level_color.0, level_color.1, level_color.2, 0.6),
                        filled: false,
                        stroke_width: 1,
                    },
                ))?;
            }
        }

        // 2. EMA 20（オレンジ）の描画
        let ema20_points: Vec<(usize, f64)> = ema20_slice
            .iter()
            .enumerate()
            .filter_map(|(i, &opt_val)| opt_val.map(|v| (i, v)))
            .collect();
        if !ema20_points.is_empty() {
            chart
                .draw_series(LineSeries::new(
                    ema20_points,
                    ShapeStyle {
                        color: RGBAColor(255, 152, 0, 0.9), // オレンジ
                        filled: false,
                        stroke_width: 2,
                    },
                ))?
                .label("EMA 20")
                .legend(|(x, y)| {
                    PathElement::new(
                        vec![(x, y), (x + 20, y)],
                        ShapeStyle {
                            color: RGBAColor(255, 152, 0, 1.0),
                            filled: false,
                            stroke_width: 2,
                        },
                    )
                });
        }

        // 3. EMA 50（ライトブルー）の描画
        let ema50_points: Vec<(usize, f64)> = ema50_slice
            .iter()
            .enumerate()
            .filter_map(|(i, &opt_val)| opt_val.map(|v| (i, v)))
            .collect();
        if !ema50_points.is_empty() {
            chart
                .draw_series(LineSeries::new(
                    ema50_points,
                    ShapeStyle {
                        color: RGBAColor(33, 150, 243, 0.9), // ブルー
                        filled: false,
                        stroke_width: 2,
                    },
                ))?
                .label("EMA 50")
                .legend(|(x, y)| {
                    PathElement::new(
                        vec![(x, y), (x + 20, y)],
                        ShapeStyle {
                            color: RGBAColor(33, 150, 243, 1.0),
                            filled: false,
                            stroke_width: 2,
                        },
                    )
                });
        }

        // 4. ローソク足（CandleStick）の描画
        let bull_color = RGBColor(38, 166, 154); // エメラルドグリーン
        let bear_color = RGBColor(239, 83, 80); // レッド

        let candle_width = ((self.config.width as f64 / 2.0) / (num_bars as f64 * 1.5))
            .max(4.0)
            .min(14.0) as u32;

        chart.draw_series(display_bars.iter().enumerate().map(|(idx, bar)| {
            let gain_style = ShapeStyle {
                color: RGBAColor(bull_color.0, bull_color.1, bull_color.2, 0.95),
                filled: true,
                stroke_width: 1,
            };
            let loss_style = ShapeStyle {
                color: RGBAColor(bear_color.0, bear_color.1, bear_color.2, 0.95),
                filled: true,
                stroke_width: 1,
            };

            CandleStick::new(
                idx,
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                gain_style,
                loss_style,
                candle_width,
            )
        }))?;

        // 5. 5Mトリガー足パネル専用のハイライト演出
        if is_trigger_panel && !display_bars.is_empty() {
            let latest_idx = num_bars - 1;
            let latest_bar = &display_bars[latest_idx];

            // 現在価格の水平点線（イエロー）
            let now_price = current_price.unwrap_or(latest_bar.close);
            chart.draw_series(LineSeries::new(
                vec![(0, now_price), (num_bars, now_price)],
                ShapeStyle {
                    color: RGBAColor(255, 235, 59, 0.8), // イエロー
                    filled: false,
                    stroke_width: 1,
                },
            ))?;

            // トリガー検出時のハイライトボックス
            if let Some(pat) = pattern_detected {
                if pat != "NONE" {
                    let box_top = latest_bar.high + (pip_size * 2.0);
                    let box_bottom = latest_bar.low - (pip_size * 2.0);
                    let highlight_color = if pat.contains("BULLISH") || pat.contains("SUPPORT") {
                        RGBColor(0, 230, 118) // 明るいグリーン
                    } else {
                        RGBColor(255, 82, 82) // 明るいレッド
                    };

                    chart.draw_series(std::iter::once(PathElement::new(
                        vec![
                            (latest_idx.saturating_sub(1), box_top),
                            (latest_idx + 1, box_top),
                            (latest_idx + 1, box_bottom),
                            (latest_idx.saturating_sub(1), box_bottom),
                            (latest_idx.saturating_sub(1), box_top),
                        ],
                        ShapeStyle {
                            color: RGBAColor(
                                highlight_color.0,
                                highlight_color.1,
                                highlight_color.2,
                                0.85,
                            ),
                            filled: false,
                            stroke_width: 2,
                        },
                    )))?;
                }
            }
        }

        // 凡例の描画
        chart
            .configure_series_labels()
            .background_style(RGBAColor(19, 23, 34, 0.8))
            .border_style(RGBColor(50, 56, 72))
            .label_font(("sans-serif", 10).into_font().color(&RGBColor(200, 205, 215)))
            .position(SeriesLabelPosition::UpperLeft)
            .draw()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn make_sample_bars(count: usize, base_price: f64) -> Vec<CandleBar> {
        let now = Utc::now();
        (0..count)
            .map(|i| {
                let offset = (i as f64 * 0.1).sin() * 0.5;
                let open = base_price + offset;
                let close = open + if i % 2 == 0 { 0.15 } else { -0.12 };
                let high = open.max(close) + 0.10;
                let low = open.min(close) - 0.10;
                CandleBar {
                    timestamp: now + Duration::minutes((i as i64) * 5),
                    open,
                    high,
                    low,
                    close,
                    volume: 150,
                }
            })
            .collect()
    }

    #[test]
    fn test_chart_plotting_to_file() {
        let bars_4h = make_sample_bars(30, 154.0);
        let bars_1h = make_sample_bars(35, 154.2);
        let bars_15m = make_sample_bars(40, 154.3);
        let bars_5m = make_sample_bars(45, 154.35);

        let data = MultiTimeframeChartData {
            pair: "USDJPY",
            bars_4h: &bars_4h,
            bars_1h: &bars_1h,
            bars_15m: &bars_15m,
            bars_5m: &bars_5m,
            key_levels_4h: vec![155.0, 153.8],
            key_levels_1h: vec![154.5, 154.0],
            current_price: Some(154.35),
            pattern_detected: Some("SUPPORT_REJECTION_PINBAR"),
        };

        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("test_chart_output_{}.png", std::process::id()));

        let plotter = ChartPlotter::with_default();
        let res = plotter.plot_to_file(&data, &path);
        assert!(res.is_ok(), "plot_to_file failed: {:?}", res.err());
        assert!(path.exists(), "PNG file was not created");

        let file_bytes = std::fs::read(&path).unwrap();
        assert!(file_bytes.len() > 1000, "PNG file is too small");
        // PNGマジックナンバーの確認: \x89PNG\r\n\x1a\n
        assert_eq!(&file_bytes[0..8], b"\x89PNG\r\n\x1a\n");

        let _ = std::fs::remove_file(path);
    }
}
