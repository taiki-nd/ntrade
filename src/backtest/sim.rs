//! 約定シミュレーター: 執行用の足を1本ずつ進め、注文の約定・SL/TP・スワップ・手数料を口座に反映する。
//!
//! 足の価格は Bid とみなす（cTrader の足は Bid）。Ask = Bid + スプレッド。
//! 買いは Ask で建てて Bid で手仕舞い、売りは Bid で建てて Ask で手仕舞う。
//!
//! 1本の足の中の値動きの順序は分からないので、迷ったら不利な側に倒す:
//! - SL と TP の両方に届いた足は SL
//! - 予約注文が約定した足では TP を見ない（約定前の値動きかもしれないため）。SL は見る
//! - 窓を空けて SL を越えたら始値で決済（滑る）。TP は TP の価格で決済（有利な滑りは見込まない）
//! - 指値は指値の価格で約定（有利な窓開けは見込まない）。逆指値は窓を空けたら始値で約定し、さらに滑りを足す

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc, Weekday};
use serde::{Deserialize, Serialize};

use crate::ctrader::{CandleBar, PendingKind};
use crate::strategy::{OrderView, PositionView, Side};

/// 取引コスト（口座通貨は `BacktestConfig::account_currency`）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Costs {
    /// スプレッド（pips）。ゼロスプレッド口座でも実際には少し開くので 0 より大きめにしておく
    pub spread_pips: f64,
    /// 成行・逆指値・SL の約定の滑り（pips、不利な方向）
    pub slippage_pips: f64,
    /// 手数料（1 lot・片道あたり、口座通貨）。建てるときと手仕舞うときにそれぞれ掛かる
    pub commission_per_lot: f64,
    /// スワップ（1 lot・1 日あたり、口座通貨。受け取りは正、支払いは負）。水曜は 3 日分
    pub swap_long_per_lot: f64,
    pub swap_short_per_lot: f64,
    /// ロールオーバーの時刻（UTC の時）
    pub rollover_hour_utc: u32,
}

impl Default for Costs {
    fn default() -> Self {
        Self {
            spread_pips: 0.2,
            slippage_pips: 0.1,
            commission_per_lot: 0.0,
            swap_long_per_lot: 0.0,
            swap_short_per_lot: 0.0,
            rollover_hour_utc: 21,
        }
    }
}

/// 決済の理由
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    /// 戦略の決済シグナル
    Strategy,
    /// 期間の終わりで強制決済
    EndOfTest,
}

impl ExitReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TakeProfit => "TAKE_PROFIT",
            Self::StopLoss => "STOP_LOSS",
            Self::Strategy => "STRATEGY",
            Self::EndOfTest => "END_OF_TEST",
        }
    }
}

/// 1 回の取引（建てて手仕舞うまで）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestTrade {
    pub id: String,
    pub side: Side,
    pub volume_lots: f64,
    /// MARKET / LIMIT / STOP
    pub entry_kind: String,
    pub entry_time: DateTime<Utc>,
    pub entry_price: f64,
    pub exit_time: DateTime<Utc>,
    pub exit_price: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub pnl_pips: f64,
    /// 値幅の損益（口座通貨）
    pub gross: f64,
    /// 手数料（往復、口座通貨。正の値 = 支払い）
    pub commission: f64,
    /// スワップ（口座通貨。受け取りは正）
    pub swap: f64,
    /// 純損益 = gross + swap - commission
    pub net: f64,
    pub exit_reason: ExitReason,
    pub entry_reason: String,
    pub exit_note: String,
    /// 保有中の最大逆行・最大順行（pips）
    pub mae_pips: f64,
    pub mfe_pips: f64,
}

#[derive(Debug, Clone)]
struct SimPosition {
    id: String,
    side: Side,
    lots: f64,
    entry_kind: &'static str,
    entry_time: DateTime<Utc>,
    entry_price: f64,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    reason: String,
    commission: f64,
    swap: f64,
    mae: f64,
    mfe: f64,
    /// この足で予約注文が約定した（この足では TP を見ない）
    filled_mid_bar: bool,
    /// 戦略が決済を求めた理由（次の足の始値で決済）
    closing: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum SimOrderKind {
    Market,
    Pending(PendingKind, f64),
}

#[derive(Debug, Clone)]
pub(crate) struct SimOrder {
    pub id: String,
    pub side: Side,
    pub kind: SimOrderKind,
    pub lots: f64,
    pub stop_loss: f64,
    pub take_profit: Option<f64>,
    pub created: DateTime<Utc>,
    pub expires: Option<DateTime<Utc>>,
    pub reason: String,
}

/// 資産の推移の 1 点
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EquityPoint {
    pub time: DateTime<Utc>,
    pub balance: f64,
    pub equity: f64,
}

pub(crate) struct SimBroker {
    pip: f64,
    spread: f64,
    slippage: f64,
    lot_units: f64,
    costs: Costs,
    pub balance: f64,
    positions: Vec<SimPosition>,
    orders: Vec<SimOrder>,
    pub trades: Vec<BacktestTrade>,
    next_id: u64,
    next_rollover: Option<DateTime<Utc>>,
    day: Option<NaiveDate>,
    day_start_balance: f64,
    /// 直近の足の終値（Bid）と、その時点の建値通貨 → 口座通貨のレート
    last_close: f64,
    last_rate: f64,
}

impl SimBroker {
    pub fn new(pip: f64, lot_units: f64, costs: &Costs, balance: f64) -> Self {
        Self {
            pip,
            spread: costs.spread_pips * pip,
            slippage: costs.slippage_pips * pip,
            lot_units,
            costs: costs.clone(),
            balance,
            positions: Vec::new(),
            orders: Vec::new(),
            trades: Vec::new(),
            next_id: 0,
            next_rollover: None,
            day: None,
            day_start_balance: balance,
            last_close: 0.0,
            last_rate: 1.0,
        }
    }

    fn new_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}-{}", self.next_id)
    }

    // ---------------------------------------------------------------- 状態

    pub fn position_views(&self) -> Vec<PositionView> {
        self.positions
            .iter()
            .filter(|p| p.closing.is_none())
            .map(|p| PositionView {
                id: p.id.clone(),
                side: p.side,
                volume_lots: p.lots,
                entry_price: p.entry_price,
                stop_loss: p.stop_loss,
                take_profit: p.take_profit,
                open_time: p.entry_time,
            })
            .collect()
    }

    pub fn order_views(&self) -> Vec<OrderView> {
        self.orders
            .iter()
            .filter_map(|o| match o.kind {
                SimOrderKind::Pending(kind, price) => Some(OrderView {
                    id: o.id.clone(),
                    side: o.side,
                    kind,
                    price,
                    stop_loss: Some(o.stop_loss),
                    take_profit: o.take_profit,
                    created_at: o.created,
                    expires_at: o.expires,
                }),
                SimOrderKind::Market => None,
            })
            .collect()
    }

    /// リスクチェック用の建玉数（決済待ちは除く）と予約注文数（成行の約定待ちは建玉に数える）
    pub fn exposure(&self) -> (usize, usize) {
        let market = self.orders.iter().filter(|o| matches!(o.kind, SimOrderKind::Market)).count();
        let positions = self.positions.iter().filter(|p| p.closing.is_none()).count();
        (positions + market, self.orders.len() - market)
    }

    /// 当日（UTC）の確定損益（資金比 %）
    pub fn daily_pnl_pct(&self, now: DateTime<Utc>) -> f64 {
        if self.day != Some(now.date_naive()) || self.day_start_balance <= 0.0 {
            return 0.0;
        }
        (self.balance - self.day_start_balance) / self.day_start_balance * 100.0
    }

    pub fn equity(&self) -> f64 {
        let open: f64 = self
            .positions
            .iter()
            .map(|p| {
                let exit = match p.side {
                    Side::Buy => self.last_close,
                    Side::Sell => self.last_close + self.spread,
                };
                (exit - p.entry_price) * p.side.sign() * self.lot_units * p.lots * self.last_rate + p.swap - p.commission
            })
            .sum();
        self.balance + open
    }

    // ---------------------------------------------------------------- 操作

    pub fn submit(&mut self, order: SimOrder) {
        self.orders.push(order);
    }

    pub fn order_id(&mut self) -> String {
        self.new_id("ord")
    }

    pub fn request_close(&mut self, id: &str, reason: &str) -> bool {
        match self.positions.iter_mut().find(|p| p.id == id && p.closing.is_none()) {
            Some(p) => {
                p.closing = Some(reason.to_string());
                true
            }
            None => false,
        }
    }

    pub fn cancel(&mut self, id: &str) -> bool {
        let before = self.orders.len();
        self.orders.retain(|o| o.id != id);
        self.orders.len() != before
    }

    pub fn modify(&mut self, id: &str, stop_loss: Option<f64>, take_profit: Option<f64>) -> bool {
        match self.positions.iter_mut().find(|p| p.id == id) {
            Some(p) => {
                if stop_loss.is_some() {
                    p.stop_loss = stop_loss;
                }
                if take_profit.is_some() {
                    p.take_profit = take_profit;
                }
                true
            }
            None => false,
        }
    }

    // ---------------------------------------------------------------- 足の処理

    /// 執行用の足を 1 本進める。`rate` は建値通貨 → 口座通貨のレート
    pub fn process_bar(&mut self, bar: &CandleBar, rate: f64) {
        self.last_rate = rate;
        self.roll_day(bar.timestamp);
        self.charge_swap(bar.timestamp);
        for p in self.positions.iter_mut() {
            p.filled_mid_bar = false;
        }

        // 1. 期限切れの予約注文を外す（期限ちょうどに始まる足では約定させない）
        self.orders.retain(|o| o.expires.is_none_or(|e| e > bar.timestamp));

        // 2. 戦略が求めた決済は始値で
        let closing: Vec<(String, String)> =
            self.positions.iter().filter_map(|p| p.closing.clone().map(|r| (p.id.clone(), r))).collect();
        for (id, note) in closing {
            if let Some(i) = self.positions.iter().position(|p| p.id == id) {
                let p = &self.positions[i];
                let price = match p.side {
                    Side::Buy => bar.open - self.slippage,
                    Side::Sell => bar.open + self.spread + self.slippage,
                };
                self.close_at(i, price, bar.timestamp, ExitReason::Strategy, note);
            }
        }

        // 3. 注文の約定
        let orders = std::mem::take(&mut self.orders);
        for o in orders {
            match self.fill_price(&o, bar) {
                Some(price) => self.open(o, price, bar.timestamp),
                None => self.orders.push(o),
            }
        }

        // 4. SL / TP
        let mut i = 0;
        while i < self.positions.len() {
            if let Some((price, reason)) = self.exit_hit(&self.positions[i], bar) {
                self.close_at(i, price, bar.timestamp, reason, String::new());
                continue;
            }
            i += 1;
        }

        // 5. 最大逆行・順行
        for p in self.positions.iter_mut() {
            let (fav, adv) = match p.side {
                Side::Buy => (bar.high - p.entry_price, p.entry_price - bar.low),
                Side::Sell => (p.entry_price - (bar.low + self.spread), (bar.high + self.spread) - p.entry_price),
            };
            p.mfe = p.mfe.max(fav);
            p.mae = p.mae.max(adv);
        }
        self.last_close = bar.close;
    }

    /// 約定価格。約定しなければ None
    fn fill_price(&self, o: &SimOrder, bar: &CandleBar) -> Option<f64> {
        let (ask_open, ask_high, ask_low) = (bar.open + self.spread, bar.high + self.spread, bar.low + self.spread);
        match (o.side, &o.kind) {
            (Side::Buy, SimOrderKind::Market) => Some(ask_open + self.slippage),
            (Side::Sell, SimOrderKind::Market) => Some(bar.open - self.slippage),
            (Side::Buy, SimOrderKind::Pending(PendingKind::Limit, p)) => (ask_low <= *p).then_some(*p),
            (Side::Sell, SimOrderKind::Pending(PendingKind::Limit, p)) => (bar.high >= *p).then_some(*p),
            (Side::Buy, SimOrderKind::Pending(PendingKind::Stop, p)) => (ask_high >= *p).then(|| ask_open.max(*p) + self.slippage),
            (Side::Sell, SimOrderKind::Pending(PendingKind::Stop, p)) => (bar.low <= *p).then(|| bar.open.min(*p) - self.slippage),
        }
    }

    fn open(&mut self, o: SimOrder, price: f64, at: DateTime<Utc>) {
        let pending = matches!(o.kind, SimOrderKind::Pending(..));
        let entry_kind = match o.kind {
            SimOrderKind::Market => "MARKET",
            SimOrderKind::Pending(PendingKind::Limit, _) => "LIMIT",
            SimOrderKind::Pending(PendingKind::Stop, _) => "STOP",
        };
        let commission = self.costs.commission_per_lot * o.lots;
        let id = self.new_id("pos");
        self.positions.push(SimPosition {
            id,
            side: o.side,
            lots: o.lots,
            entry_kind,
            entry_time: at,
            entry_price: price,
            stop_loss: Some(o.stop_loss),
            take_profit: o.take_profit,
            reason: o.reason,
            commission,
            swap: 0.0,
            mae: 0.0,
            mfe: 0.0,
            filled_mid_bar: pending,
            closing: None,
        });
    }

    /// この足で SL / TP に届いたか（決済価格と理由）
    fn exit_hit(&self, p: &SimPosition, bar: &CandleBar) -> Option<(f64, ExitReason)> {
        let s = self.spread;
        // 手仕舞いの価格は、買いは Bid・売りは Ask
        let (open, high, low) = match p.side {
            Side::Buy => (bar.open, bar.high, bar.low),
            Side::Sell => (bar.open + s, bar.high + s, bar.low + s),
        };
        let sign = p.side.sign();
        let sl_hit = p.stop_loss.is_some_and(|sl| if sign > 0.0 { low <= sl } else { high >= sl });
        if let (true, Some(sl)) = (sl_hit, p.stop_loss) {
            // 窓を空けて越えていたら始値で
            let gapped = if sign > 0.0 { open <= sl } else { open >= sl };
            let base = if gapped { open } else { sl };
            return Some((base - sign * self.slippage, ExitReason::StopLoss));
        }
        if p.filled_mid_bar {
            return None;
        }
        let tp = p.take_profit?;
        let tp_hit = if sign > 0.0 { high >= tp } else { low <= tp };
        tp_hit.then_some((tp, ExitReason::TakeProfit))
    }

    fn close_at(&mut self, i: usize, price: f64, at: DateTime<Utc>, reason: ExitReason, note: String) {
        let p = self.positions.remove(i);
        let sign = p.side.sign();
        let gross = (price - p.entry_price) * sign * self.lot_units * p.lots * self.last_rate;
        let commission = p.commission + self.costs.commission_per_lot * p.lots;
        let net = gross + p.swap - commission;
        self.balance += gross + p.swap - commission;
        self.trades.push(BacktestTrade {
            id: p.id,
            side: p.side,
            volume_lots: p.lots,
            entry_kind: p.entry_kind.to_string(),
            entry_time: p.entry_time,
            entry_price: p.entry_price,
            exit_time: at,
            exit_price: price,
            stop_loss: p.stop_loss,
            take_profit: p.take_profit,
            pnl_pips: (price - p.entry_price) * sign / self.pip,
            gross,
            commission,
            swap: p.swap,
            net,
            exit_reason: reason,
            entry_reason: p.reason,
            exit_note: note,
            mae_pips: p.mae / self.pip,
            mfe_pips: p.mfe / self.pip,
        });
    }

    /// 残っている建玉を最後の終値で決済し、予約注文を消す
    pub fn close_all(&mut self, at: DateTime<Utc>) {
        while !self.positions.is_empty() {
            let price = match self.positions[0].side {
                Side::Buy => self.last_close,
                Side::Sell => self.last_close + self.spread,
            };
            self.close_at(0, price, at, ExitReason::EndOfTest, String::new());
        }
        self.orders.clear();
    }

    fn roll_day(&mut self, t: DateTime<Utc>) {
        let d = t.date_naive();
        if self.day != Some(d) {
            self.day = Some(d);
            self.day_start_balance = self.balance;
        }
    }

    /// ロールオーバーをまたいだ建玉にスワップを付ける。土日のロールオーバーは無く、水曜は 3 日分
    fn charge_swap(&mut self, t: DateTime<Utc>) {
        let hour = self.costs.rollover_hour_utc.min(23);
        let next = *self.next_rollover.get_or_insert_with(|| first_rollover(t, hour));
        if t < next {
            return;
        }
        let mut r = next;
        while r <= t {
            let days = match r.weekday() {
                Weekday::Sat | Weekday::Sun => 0.0,
                Weekday::Wed => 3.0,
                _ => 1.0,
            };
            if days > 0.0 {
                for p in self.positions.iter_mut().filter(|p| p.entry_time < r) {
                    let per_lot = match p.side {
                        Side::Buy => self.costs.swap_long_per_lot,
                        Side::Sell => self.costs.swap_short_per_lot,
                    };
                    p.swap += per_lot * p.lots * days;
                }
            }
            r += Duration::days(1);
        }
        self.next_rollover = Some(r);
    }
}

fn first_rollover(t: DateTime<Utc>, hour: u32) -> DateTime<Utc> {
    let today = t.date_naive().and_hms_opt(hour, 0, 0).map(|n| n.and_utc()).unwrap_or(t);
    if today >= t {
        today
    } else {
        today + Duration::days(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn bar(min: i64, o: f64, h: f64, l: f64, c: f64) -> CandleBar {
        let t0 = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap(); // 火曜
        CandleBar { timestamp: t0 + Duration::minutes(min), open: o, high: h, low: l, close: c, volume: 1 }
    }

    fn costs() -> Costs {
        Costs { spread_pips: 0.0, slippage_pips: 0.0, commission_per_lot: 0.0, ..Costs::default() }
    }

    fn broker(c: &Costs) -> SimBroker {
        // USDJPY: 1 lot = 100,000 通貨、JPY 口座（レート 1）
        SimBroker::new(0.01, 100_000.0, c, 1_000_000.0)
    }

    fn order(b: &mut SimBroker, side: Side, kind: SimOrderKind, sl: f64, tp: Option<f64>) {
        let id = b.order_id();
        let t = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
        b.submit(SimOrder { id, side, kind, lots: 0.1, stop_loss: sl, take_profit: tp, created: t, expires: None, reason: "t".into() });
    }

    #[test]
    fn market_buy_fills_at_the_ask_open_and_hits_tp() {
        let c = Costs { spread_pips: 0.2, ..costs() };
        let mut b = broker(&c);
        order(&mut b, Side::Buy, SimOrderKind::Market, 154.00, Some(154.40));
        b.process_bar(&bar(0, 154.20, 154.30, 154.10, 154.25), 1.0);
        assert_eq!(b.position_views()[0].entry_price, 154.202);
        b.process_bar(&bar(5, 154.25, 154.45, 154.20, 154.40), 1.0);
        let t = &b.trades[0];
        assert_eq!(t.exit_reason, ExitReason::TakeProfit);
        assert_eq!(t.exit_price, 154.40);
        // (154.40 - 154.202) × 100,000 × 0.1 = 1,980 円
        assert!((t.net - 1980.0).abs() < 1e-6, "{}", t.net);
        assert!((b.balance - 1_001_980.0).abs() < 1e-6);
    }

    #[test]
    fn same_bar_sl_and_tp_is_a_loss_and_gaps_slip() {
        let mut b = broker(&costs());
        order(&mut b, Side::Sell, SimOrderKind::Market, 154.30, Some(154.00));
        b.process_bar(&bar(0, 154.20, 154.22, 154.18, 154.20), 1.0);
        b.process_bar(&bar(5, 154.20, 154.35, 153.95, 154.10), 1.0);
        assert_eq!(b.trades[0].exit_reason, ExitReason::StopLoss);
        assert_eq!(b.trades[0].exit_price, 154.30);

        // 窓を空けて SL を越えたら始値で
        let mut b = broker(&costs());
        order(&mut b, Side::Buy, SimOrderKind::Market, 154.00, None);
        b.process_bar(&bar(0, 154.20, 154.22, 154.18, 154.20), 1.0);
        b.process_bar(&bar(5, 153.80, 153.90, 153.70, 153.85), 1.0);
        assert_eq!(b.trades[0].exit_price, 153.80);
        assert!((b.trades[0].pnl_pips - (-40.0)).abs() < 1e-9);
    }

    #[test]
    fn pending_orders_fill_conservatively_and_expire() {
        // 買い指値 154.10。約定した足では TP を見ない
        let mut b = broker(&costs());
        order(&mut b, Side::Buy, SimOrderKind::Pending(PendingKind::Limit, 154.10), 154.00, Some(154.30));
        b.process_bar(&bar(0, 154.20, 154.35, 154.08, 154.30), 1.0);
        assert!(b.trades.is_empty());
        assert_eq!(b.position_views()[0].entry_price, 154.10);
        b.process_bar(&bar(5, 154.30, 154.35, 154.25, 154.30), 1.0);
        assert_eq!(b.trades[0].exit_reason, ExitReason::TakeProfit);

        // 売り逆指値 154.00 を窓で抜けたら始値で約定
        let mut b = broker(&costs());
        order(&mut b, Side::Sell, SimOrderKind::Pending(PendingKind::Stop, 154.00), 154.20, None);
        b.process_bar(&bar(0, 153.92, 153.96, 153.90, 153.92), 1.0);
        assert_eq!(b.position_views()[0].entry_price, 153.92);

        // 期限が来たら約定させない
        let mut b = broker(&costs());
        let id = b.order_id();
        b.submit(SimOrder {
            id,
            side: Side::Buy,
            kind: SimOrderKind::Pending(PendingKind::Stop, 154.50),
            lots: 0.1,
            stop_loss: 154.0,
            take_profit: None,
            created: bar(0, 0.0, 0.0, 0.0, 0.0).timestamp,
            expires: Some(bar(5, 0.0, 0.0, 0.0, 0.0).timestamp),
            reason: "t".into(),
        });
        b.process_bar(&bar(0, 154.20, 154.30, 154.10, 154.20), 1.0);
        b.process_bar(&bar(5, 154.20, 154.60, 154.10, 154.50), 1.0);
        assert!(b.position_views().is_empty() && b.order_views().is_empty());
    }

    #[test]
    fn strategy_close_happens_at_the_next_open_with_commission() {
        let c = Costs { commission_per_lot: 800.0, ..costs() };
        let mut b = broker(&c);
        order(&mut b, Side::Buy, SimOrderKind::Market, 150.00, None);
        b.process_bar(&bar(0, 154.20, 154.30, 154.10, 154.25), 1.0);
        let id = b.position_views()[0].id.clone();
        assert!(b.request_close(&id, "反対のクロス"));
        // 決済待ちの建玉は戦略から見えない
        assert!(b.position_views().is_empty());
        b.process_bar(&bar(5, 154.40, 154.50, 154.30, 154.45), 1.0);
        let t = &b.trades[0];
        assert_eq!((t.exit_reason, t.exit_price), (ExitReason::Strategy, 154.40));
        // 往復の手数料 800 × 0.1 × 2 = 160 円
        assert!((t.commission - 160.0).abs() < 1e-9);
        assert!((t.net - (2000.0 - 160.0)).abs() < 1e-6);
    }

    #[test]
    fn swap_is_charged_at_rollover_and_tripled_on_wednesday() {
        let c = Costs { swap_long_per_lot: 100.0, ..costs() };
        let mut b = broker(&c);
        order(&mut b, Side::Buy, SimOrderKind::Market, 100.0, None);
        b.process_bar(&bar(0, 154.20, 154.30, 154.10, 154.20), 1.0); // 火曜 09:00
        // 火曜 21:00（1 日分）と水曜 21:00（3 日分）をまたぐ
        b.process_bar(&bar(60 * 37, 154.20, 154.30, 154.10, 154.20), 1.0); // 水曜 22:00
        b.close_all(bar(60 * 37, 0.0, 0.0, 0.0, 0.0).timestamp);
        assert!((b.trades[0].swap - 100.0 * 0.1 * 4.0).abs() < 1e-9, "{}", b.trades[0].swap);
    }

    #[test]
    fn daily_pnl_resets_at_utc_midnight() {
        let mut b = broker(&costs());
        order(&mut b, Side::Buy, SimOrderKind::Market, 150.0, Some(154.30));
        b.process_bar(&bar(0, 154.20, 154.25, 154.15, 154.20), 1.0);
        b.process_bar(&bar(5, 154.20, 154.35, 154.15, 154.30), 1.0);
        let t = bar(5, 0.0, 0.0, 0.0, 0.0).timestamp;
        assert!((b.daily_pnl_pct(t) - 0.1).abs() < 1e-9);
        assert_eq!(b.daily_pnl_pct(t + Duration::days(1)), 0.0);
    }
}
