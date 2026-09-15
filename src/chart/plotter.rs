//! 時間足ごとに1枚のローソク足チャートPNGを生成する。
//!
//! 描くのは「事実」だけ: ローソク足、EMA20/50、価格ラベル付きの水平線、現在価格。
//! パターン名、トリガー枠、方向を示唆する強調は描かない。

use anyhow::{Context, Result};
use plotters::prelude::*;
use std::fs;
use std::path::Path;

use crate::ctrader::CandleBar;
use crate::snapshot::measures::{calculate_ema, get_pip_size, price_digits};

/// チャート生成設定
#[derive(Debug, Clone)]
pub struct ChartPlotterConfig {
    pub width: u32,
    pub height: u32,
    pub bars_to_display: usize,
}

impl Default for ChartPlotterConfig {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
            bars_to_display: 60,
        }
    }
}

/// 水平線として描く価格。ラベルは価格と出所（例: "PDH"）のみで、支持/抵抗の語は含めない。
#[derive(Debug, Clone, PartialEq)]
pub struct PriceLevel {
    pub price: f64,
    pub tag: String,
}

/// 1時間足分の描画データ
pub struct TimeframeChart<'a> {
    pub pair: &'a str,
    pub timeframe: &'a str,
    pub bars: &'a [CandleBar],
    pub levels: &'a [PriceLevel],
    pub current_price: Option<f64>,
}

/// ローソク足チャート画像ジェネレータ
pub struct ChartPlotter {
    config: ChartPlotterConfig,
}

impl ChartPlotter {
    pub fn new(config: ChartPlotterConfig) -> Self {
        Self { config }
    }

    pub fn with_default() -> Self {
        Self::new(ChartPlotterConfig::default())
    }

    pub fn config(&self) -> &ChartPlotterConfig {
        &self.config
    }

    /// チャート画像をPNGファイルとして保存する
    pub fn plot_to_file(&self, chart: &TimeframeChart, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create chart directory: {:?}", parent))?;
        }

        let root = BitMapBackend::new(path, (self.config.width, self.config.height))
            .into_drawing_area();
        self.render(&root, chart)
            .map_err(|e| anyhow::anyhow!("Chart rendering failed: {}", e))?;
        root.present()
            .with_context(|| format!("Failed to save chart image to {:?}", path))?;
        Ok(())
    }

    fn render<DB: DrawingBackend>(
        &self,
        root: &DrawingArea<DB, plotters::coord::Shift>,
        chart: &TimeframeChart,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        let bg_color = RGBColor(19, 23, 34);
        root.fill(&bg_color)?;

        let pip_size = get_pip_size(chart.pair);
        let digits = price_digits(chart.pair);

        let all_bars = chart.bars;
        let start_idx = all_bars.len().saturating_sub(self.config.bars_to_display);
        let display_bars = &all_bars[start_idx..];
        if display_bars.is_empty() {
            return Ok(());
        }

        let ema20_all = calculate_ema(all_bars, 20);
        let ema50_all = calculate_ema(all_bars, 50);
        let ema20_slice = &ema20_all[start_idx..];
        let ema50_slice = &ema50_all[start_idx..];

        let mut min_p = display_bars.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        let mut max_p = display_bars.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
        for &e in ema20_slice.iter().chain(ema50_slice.iter()).flatten() {
            min_p = min_p.min(e);
            max_p = max_p.max(e);
        }
        // 表示範囲に近い水平線は範囲に含める（遠すぎる線で縦軸が潰れるのを防ぐ）
        let bar_range = (max_p - min_p).max(pip_size * 5.0);
        for lvl in chart.levels {
            if lvl.price > max_p && lvl.price - max_p < bar_range * 0.5 {
                max_p = lvl.price;
            }
            if lvl.price < min_p && min_p - lvl.price < bar_range * 0.5 {
                min_p = lvl.price;
            }
        }

        let range = (max_p - min_p).max(pip_size * 5.0);
        let y_min = min_p - range * 0.06;
        let y_max = max_p + range * 0.06;
        let num_bars = display_bars.len();
        // 右側に価格ラベル用の余白（ラベルが画面外にはみ出さないよう十分に取る）
        let x_max = num_bars + (num_bars / 4).max(12);

        let last = display_bars.last().unwrap();
        let title = format!(
            "{}  {}   last bar: {} UTC",
            chart.pair,
            chart.timeframe,
            last.timestamp.format("%Y-%m-%d %H:%M")
        );

        let mut cc = ChartBuilder::on(root)
            .caption(title, ("sans-serif", 18).into_font().color(&RGBColor(240, 242, 245)))
            .margin(10)
            .x_label_area_size(28)
            .y_label_area_size(80)
            .build_cartesian_2d(0..x_max, y_min..y_max)?;

        let axis_color = RGBColor(70, 77, 95);
        let grid_bold = RGBColor(35, 41, 55);
        let grid_light = RGBColor(25, 30, 42);
        let label_color = RGBColor(160, 168, 185);
        let is_intraday_short = chart.timeframe == "5M" || chart.timeframe == "15M";

        cc.configure_mesh()
            .axis_style(axis_color)
            .bold_line_style(grid_bold)
            .light_line_style(grid_light)
            .label_style(("sans-serif", 12).into_font().color(&label_color))
            .x_labels(10)
            .y_labels(10)
            .x_label_formatter(&|&x| {
                if x < display_bars.len() {
                    if is_intraday_short {
                        display_bars[x].timestamp.format("%H:%M").to_string()
                    } else {
                        display_bars[x].timestamp.format("%m-%d %H:%M").to_string()
                    }
                } else {
                    String::new()
                }
            })
            .y_label_formatter(&move |&y| format!("{:.*}", digits, y))
            .draw()?;

        // 1. 水平線（価格ラベル付き）。近接する線のラベルは1つに束ねて重なりを防ぐ。
        let level_color = RGBAColor(186, 104, 200, 0.55);
        let mut visible: Vec<&PriceLevel> = chart
            .levels
            .iter()
            .filter(|l| l.price >= y_min && l.price <= y_max)
            .collect();
        visible.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
        for lvl in &visible {
            cc.draw_series(LineSeries::new(
                vec![(0, lvl.price), (x_max, lvl.price)],
                ShapeStyle { color: level_color, filled: false, stroke_width: 1 },
            ))?;
        }
        let merge_tol = (y_max - y_min) * 0.02;
        let mut i = 0;
        while i < visible.len() {
            let mut j = i;
            while j + 1 < visible.len() && visible[j + 1].price - visible[i].price < merge_tol {
                j += 1;
            }
            let group = &visible[i..=j];
            let mut parts: Vec<String> = group
                .iter()
                .take(2)
                .map(|l| format!("{:.*} {}", digits, l.price, l.tag))
                .collect();
            if group.len() > 2 {
                parts.push(format!("+{}", group.len() - 2));
            }
            let text = parts.join(" | ");
            let anchor = group.iter().map(|l| l.price).sum::<f64>() / group.len() as f64;
            cc.draw_series(std::iter::once(Text::new(
                text,
                (num_bars + 1, anchor),
                ("sans-serif", 10).into_font().color(&RGBColor(200, 160, 210)),
            )))?;
            i = j + 1;
        }

        // 2. EMA 20 / 50
        let ema_series = |slice: &[Option<f64>]| -> Vec<(usize, f64)> {
            slice
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.map(|p| (i, p)))
                .collect()
        };
        let ema20_points = ema_series(ema20_slice);
        if !ema20_points.is_empty() {
            cc.draw_series(LineSeries::new(
                ema20_points,
                ShapeStyle { color: RGBAColor(255, 152, 0, 0.8), filled: false, stroke_width: 1 },
            ))?
            .label("EMA 20")
            .legend(|(x, y)| {
                PathElement::new(
                    vec![(x, y), (x + 20, y)],
                    ShapeStyle { color: RGBAColor(255, 152, 0, 1.0), filled: false, stroke_width: 2 },
                )
            });
        }
        let ema50_points = ema_series(ema50_slice);
        if !ema50_points.is_empty() {
            cc.draw_series(LineSeries::new(
                ema50_points,
                ShapeStyle { color: RGBAColor(33, 150, 243, 0.8), filled: false, stroke_width: 1 },
            ))?
            .label("EMA 50")
            .legend(|(x, y)| {
                PathElement::new(
                    vec![(x, y), (x + 20, y)],
                    ShapeStyle { color: RGBAColor(33, 150, 243, 1.0), filled: false, stroke_width: 2 },
                )
            });
        }

        // 3. ローソク足
        let bull = RGBAColor(38, 166, 154, 0.95);
        let bear = RGBAColor(239, 83, 80, 0.95);
        let candle_width = ((self.config.width as f64 - 100.0) / (x_max as f64 * 1.6))
            .clamp(3.0, 16.0) as u32;

        cc.draw_series(display_bars.iter().enumerate().map(|(idx, bar)| {
            CandleStick::new(
                idx,
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                ShapeStyle { color: bull, filled: true, stroke_width: 1 },
                ShapeStyle { color: bear, filled: true, stroke_width: 1 },
                candle_width,
            )
        }))?;

        // 4. 現在価格（黄色の点線相当）
        let now_price = chart.current_price.unwrap_or(last.close);
        if now_price >= y_min && now_price <= y_max {
            cc.draw_series(LineSeries::new(
                vec![(0, now_price), (x_max, now_price)],
                ShapeStyle { color: RGBAColor(255, 235, 59, 0.7), filled: false, stroke_width: 1 },
            ))?;
            cc.draw_series(std::iter::once(Text::new(
                format!("{:.*}", digits, now_price),
                (num_bars, now_price),
                ("sans-serif", 11).into_font().color(&RGBColor(255, 235, 59)),
            )))?;
        }

        cc.configure_series_labels()
            .background_style(RGBAColor(19, 23, 34, 0.8))
            .border_style(RGBColor(50, 56, 72))
            .label_font(("sans-serif", 11).into_font().color(&RGBColor(200, 205, 215)))
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
                CandleBar {
                    timestamp: now + Duration::minutes((i as i64) * 5),
                    open,
                    high: open.max(close) + 0.10,
                    low: open.min(close) - 0.10,
                    close,
                    volume: 150,
                }
            })
            .collect()
    }

    #[test]
    fn plots_single_timeframe_png() {
        let bars = make_sample_bars(70, 154.0);
        let levels = vec![
            PriceLevel { price: 154.5, tag: "PDH".into() },
            PriceLevel { price: 153.5, tag: "PDL".into() },
        ];
        let chart = TimeframeChart {
            pair: "USDJPY",
            timeframe: "5M",
            bars: &bars,
            levels: &levels,
            current_price: Some(154.2),
        };
        let path = std::env::temp_dir().join(format!("ntrade_tf_chart_{}.png", std::process::id()));
        ChartPlotter::with_default().plot_to_file(&chart, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
        assert!(bytes.len() > 1000);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_bars_do_not_panic() {
        let chart = TimeframeChart {
            pair: "EURUSD",
            timeframe: "1H",
            bars: &[],
            levels: &[],
            current_price: None,
        };
        let path = std::env::temp_dir().join(format!("ntrade_tf_empty_{}.png", std::process::id()));
        assert!(ChartPlotter::with_default().plot_to_file(&chart, &path).is_ok());
        let _ = std::fs::remove_file(path);
    }
}
