//! FIX 4.4 メッセージのエンコード / デコード
//!
//! FIX のワイヤ形式は `tag=value` を SOH(0x01) で連結しただけの単純なもの。
//! ただし以下の3点は順序・計算規則が仕様で固定されている。
//!
//! - `8=FIX.4.4`(BeginString) → `9=<BodyLength>`(BodyLength) → `35=<MsgType>`(MsgType) の順で始まる
//! - BodyLength は BeginString と BodyLength 自身を除き、CheckSum の直前までのバイト数
//! - CheckSum は CheckSum フィールドの直前までの全バイトの総和 mod 256 を3桁ゼロ埋めにしたもの

use std::collections::HashMap;
use std::fmt;

/// フィールド区切り (SOH)
pub const SOH: u8 = 0x01;

pub const BEGIN_STRING: &str = "FIX.4.4";

/// よく使うタグ番号
pub mod tag {
    pub const BEGIN_STRING: u32 = 8;
    pub const BODY_LENGTH: u32 = 9;
    pub const MSG_TYPE: u32 = 35;
    pub const SENDER_COMP_ID: u32 = 49;
    pub const TARGET_COMP_ID: u32 = 56;
    pub const TARGET_SUB_ID: u32 = 57;
    pub const SENDER_SUB_ID: u32 = 50;
    pub const MSG_SEQ_NUM: u32 = 34;
    pub const SENDING_TIME: u32 = 52;
    pub const CHECK_SUM: u32 = 10;

    pub const ENCRYPT_METHOD: u32 = 98;
    pub const HEART_BT_INT: u32 = 108;
    pub const RESET_SEQ_NUM_FLAG: u32 = 141;
    pub const USERNAME: u32 = 553;
    pub const PASSWORD: u32 = 554;
    pub const TEST_REQ_ID: u32 = 112;
    pub const TEXT: u32 = 58;
    pub const BEGIN_SEQ_NO: u32 = 7;
    pub const END_SEQ_NO: u32 = 16;
    pub const NEW_SEQ_NO: u32 = 36;
    pub const GAP_FILL_FLAG: u32 = 123;
    pub const REF_SEQ_NUM: u32 = 45;

    pub const CL_ORD_ID: u32 = 11;
    pub const ORIG_CL_ORD_ID: u32 = 41;
    pub const ORDER_ID: u32 = 37;
    pub const SYMBOL: u32 = 55;
    pub const SIDE: u32 = 54;
    pub const TRANSACT_TIME: u32 = 60;
    pub const ORDER_QTY: u32 = 38;
    pub const ORD_TYPE: u32 = 40;
    pub const PRICE: u32 = 44;
    pub const STOP_PX: u32 = 99;
    pub const POS_MAINT_RPT_ID: u32 = 721;
    pub const DESIGNATION: u32 = 494;
    pub const EXEC_TYPE: u32 = 150;
    pub const ORD_STATUS: u32 = 39;
    pub const AVG_PX: u32 = 6;
    pub const LEAVES_QTY: u32 = 151;
    pub const CUM_QTY: u32 = 14;
    pub const LAST_QTY: u32 = 32;
    pub const ORD_REJ_REASON: u32 = 103;
}

/// MsgType(35) の値
pub mod msg_type {
    pub const HEARTBEAT: &str = "0";
    pub const TEST_REQUEST: &str = "1";
    pub const RESEND_REQUEST: &str = "2";
    pub const REJECT: &str = "3";
    pub const SEQUENCE_RESET: &str = "4";
    pub const LOGOUT: &str = "5";
    pub const LOGON: &str = "A";
    pub const NEW_ORDER_SINGLE: &str = "D";
    pub const ORDER_CANCEL_REQUEST: &str = "F";
    pub const EXECUTION_REPORT: &str = "8";
    pub const ORDER_CANCEL_REJECT: &str = "9";
    pub const BUSINESS_MESSAGE_REJECT: &str = "j";
}

/// Side(54)
pub mod side {
    pub const BUY: &str = "1";
    pub const SELL: &str = "2";
}

/// OrdType(40)
pub mod ord_type {
    pub const MARKET: &str = "1";
    pub const LIMIT: &str = "2";
    pub const STOP: &str = "3";
}

/// ExecType(150)
pub mod exec_type {
    pub const NEW: &str = "0";
    pub const CANCELED: &str = "4";
    pub const REPLACE: &str = "5";
    pub const REJECTED: &str = "8";
    pub const EXPIRED: &str = "C";
    pub const TRADE: &str = "F";
    pub const ORDER_STATUS: &str = "I";
}

/// OrdStatus(39)
pub mod ord_status {
    pub const NEW: &str = "0";
    pub const PARTIALLY_FILLED: &str = "1";
    pub const FILLED: &str = "2";
    pub const CANCELLED: &str = "4";
    pub const REJECTED: &str = "8";
    pub const EXPIRED: &str = "C";
}

/// パース済みの FIX メッセージ
///
/// FIX には繰り返しグループがあるが、cTrader の取引セッションで使うメッセージには
/// 現れないため、タグ → 最初の値の単純なマップとして保持する。
#[derive(Debug, Clone, Default)]
pub struct FixMessage {
    fields: Vec<(u32, String)>,
    index: HashMap<u32, usize>,
}

impl FixMessage {
    pub fn new(msg_type: &str) -> Self {
        let mut m = Self::default();
        m.set(tag::MSG_TYPE, msg_type);
        m
    }

    /// フィールドを追加する。同じタグが既にあれば上書きする。
    pub fn set(&mut self, tag: u32, value: impl Into<String>) -> &mut Self {
        let value = value.into();
        match self.index.get(&tag) {
            Some(&i) => self.fields[i].1 = value,
            None => {
                self.index.insert(tag, self.fields.len());
                self.fields.push((tag, value));
            }
        }
        self
    }

    pub fn get(&self, tag: u32) -> Option<&str> {
        self.index.get(&tag).map(|&i| self.fields[i].1.as_str())
    }

    pub fn get_i64(&self, tag: u32) -> Option<i64> {
        self.get(tag).and_then(|v| v.parse().ok())
    }

    pub fn get_f64(&self, tag: u32) -> Option<f64> {
        self.get(tag).and_then(|v| v.parse().ok())
    }

    pub fn msg_type(&self) -> &str {
        self.get(tag::MSG_TYPE).unwrap_or_default()
    }

    pub fn seq_num(&self) -> Option<i64> {
        self.get_i64(tag::MSG_SEQ_NUM)
    }

    /// ヘッダ（BeginString / BodyLength / MsgType）とトレーラ（CheckSum）を付けて
    /// ワイヤ形式のバイト列にする。
    ///
    /// `self` は MsgType 以降のフィールドを保持している前提。
    pub fn encode(&self) -> Vec<u8> {
        // MsgType は先頭に来る必要があるので、body は MsgType → それ以外の順で組む
        let mut body = Vec::new();
        if let Some(mt) = self.get(tag::MSG_TYPE) {
            push_field(&mut body, tag::MSG_TYPE, mt);
        }
        for (t, v) in &self.fields {
            if *t == tag::MSG_TYPE || *t == tag::BEGIN_STRING || *t == tag::BODY_LENGTH || *t == tag::CHECK_SUM {
                continue;
            }
            push_field(&mut body, *t, v);
        }

        let mut out = Vec::with_capacity(body.len() + 32);
        push_field(&mut out, tag::BEGIN_STRING, BEGIN_STRING);
        push_field(&mut out, tag::BODY_LENGTH, &body.len().to_string());
        out.extend_from_slice(&body);

        let sum: u32 = out.iter().map(|b| *b as u32).sum();
        push_field(&mut out, tag::CHECK_SUM, &format!("{:03}", sum % 256));
        out
    }

    /// ワイヤ形式のバイト列からメッセージを復元する。
    pub fn decode(raw: &[u8]) -> Result<Self, FixParseError> {
        let mut msg = Self::default();
        for part in raw.split(|b| *b == SOH) {
            if part.is_empty() {
                continue;
            }
            let eq = part.iter().position(|b| *b == b'=').ok_or(FixParseError::MalformedField)?;
            let tag: u32 = std::str::from_utf8(&part[..eq])
                .map_err(|_| FixParseError::MalformedField)?
                .parse()
                .map_err(|_| FixParseError::MalformedField)?;
            let value = String::from_utf8_lossy(&part[eq + 1..]).into_owned();
            msg.set(tag, value);
        }
        if msg.get(tag::MSG_TYPE).is_none() {
            return Err(FixParseError::MissingMsgType);
        }
        Ok(msg)
    }
}

impl fmt::Display for FixMessage {
    /// ログ用。SOH は読めないので `|` に置き換える。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = self.encode();
        write!(f, "{}", String::from_utf8_lossy(&raw).replace(SOH as char, "|"))
    }
}

fn push_field(buf: &mut Vec<u8>, tag: u32, value: &str) {
    buf.extend_from_slice(tag.to_string().as_bytes());
    buf.push(b'=');
    buf.extend_from_slice(value.as_bytes());
    buf.push(SOH);
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FixParseError {
    #[error("malformed FIX field")]
    MalformedField,
    #[error("FIX message has no MsgType(35)")]
    MissingMsgType,
}

/// 受信ストリームから完結した1メッセージを切り出す。
///
/// FIX は改行区切りではなく BodyLength で長さが決まるため、
/// `9=<len>` を読んでから必要バイト数が揃うまで待つ必要がある。
/// 切り出せた場合は (メッセージ, 消費バイト数) を返す。
pub fn extract_message(buf: &[u8]) -> Option<(Vec<u8>, usize)> {
    // 9=<len>SOH を探す
    let body_len_start = find_subslice(buf, b"9=")?;
    // "9=" が 8=FIX.4.4SOH の直後にあることを確認する（値の中の "9=" を拾わないため）
    if body_len_start == 0 || buf[body_len_start - 1] != SOH {
        return None;
    }
    let after_tag = body_len_start + 2;
    let soh = buf[after_tag..].iter().position(|b| *b == SOH)? + after_tag;
    let body_len: usize = std::str::from_utf8(&buf[after_tag..soh]).ok()?.parse().ok()?;

    // body の直後に 10=xxx SOH（7バイト）が続く
    let end = soh + 1 + body_len + 7;
    if buf.len() < end {
        return None;
    }
    Some((buf[..end].to_vec(), end))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// FIX の SendingTime(52) / TransactTime(60) 形式（UTC）
pub fn format_timestamp(t: chrono::DateTime<chrono::Utc>) -> String {
    t.format("%Y%m%d-%H:%M:%S%.3f").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipe(raw: &[u8]) -> String {
        String::from_utf8_lossy(raw).replace(SOH as char, "|")
    }

    #[test]
    fn encodes_header_body_length_and_checksum() {
        let mut m = FixMessage::new(msg_type::LOGON);
        m.set(tag::SENDER_COMP_ID, "demo.axiory.8069118")
            .set(tag::TARGET_COMP_ID, "CSERVER")
            .set(tag::MSG_SEQ_NUM, "1");
        let raw = m.encode();
        let s = pipe(&raw);

        assert!(s.starts_with("8=FIX.4.4|9="), "BeginString と BodyLength が先頭: {s}");
        assert!(s.contains("|35=A|"), "MsgType は3番目: {s}");
        assert!(s.ends_with('|'));

        // BodyLength は 35= から CheckSum 直前まで
        let body_start = raw.windows(4).position(|w| w == b"35=A").unwrap();
        let checksum_start = raw.len() - 7;
        let declared: usize = s
            .split('|')
            .find_map(|f| f.strip_prefix("9="))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(declared, checksum_start - body_start);

        // CheckSum は直前までの総和 mod 256
        let sum: u32 = raw[..checksum_start].iter().map(|b| *b as u32).sum();
        let declared_sum = s.split('|').find_map(|f| f.strip_prefix("10=")).unwrap();
        assert_eq!(declared_sum, format!("{:03}", sum % 256));
    }

    #[test]
    fn roundtrips_through_decode() {
        let mut m = FixMessage::new(msg_type::NEW_ORDER_SINGLE);
        m.set(tag::CL_ORD_ID, "ord-1")
            .set(tag::SYMBOL, "4")
            .set(tag::SIDE, side::BUY)
            .set(tag::ORDER_QTY, "1000")
            .set(tag::ORD_TYPE, ord_type::MARKET);

        let decoded = FixMessage::decode(&m.encode()).unwrap();
        assert_eq!(decoded.msg_type(), msg_type::NEW_ORDER_SINGLE);
        assert_eq!(decoded.get(tag::CL_ORD_ID), Some("ord-1"));
        assert_eq!(decoded.get_i64(tag::SYMBOL), Some(4));
        assert_eq!(decoded.get(tag::SIDE), Some(side::BUY));
        assert_eq!(decoded.get(tag::BEGIN_STRING), Some(BEGIN_STRING));
    }

    #[test]
    fn set_overwrites_existing_tag() {
        let mut m = FixMessage::new(msg_type::LOGON);
        m.set(tag::MSG_SEQ_NUM, "1").set(tag::MSG_SEQ_NUM, "2");
        assert_eq!(m.get(tag::MSG_SEQ_NUM), Some("2"));
        // 重複して出力されないこと
        assert_eq!(pipe(&m.encode()).matches("34=").count(), 1);
    }

    #[test]
    fn extracts_one_message_from_stream() {
        let mut a = FixMessage::new(msg_type::HEARTBEAT);
        a.set(tag::MSG_SEQ_NUM, "7");
        let mut b = FixMessage::new(msg_type::LOGOUT);
        b.set(tag::MSG_SEQ_NUM, "8");

        let mut stream = a.encode();
        let a_len = stream.len();
        stream.extend_from_slice(&b.encode());

        let (first, used) = extract_message(&stream).unwrap();
        assert_eq!(used, a_len);
        assert_eq!(FixMessage::decode(&first).unwrap().seq_num(), Some(7));

        let (second, _) = extract_message(&stream[used..]).unwrap();
        assert_eq!(FixMessage::decode(&second).unwrap().msg_type(), msg_type::LOGOUT);
    }

    #[test]
    fn waits_for_incomplete_message() {
        let mut m = FixMessage::new(msg_type::HEARTBEAT);
        m.set(tag::MSG_SEQ_NUM, "1");
        let raw = m.encode();
        // 途中までしか届いていないうちは切り出さない
        assert!(extract_message(&raw[..raw.len() - 3]).is_none());
        assert!(extract_message(&raw).is_some());
    }

    #[test]
    fn decode_rejects_message_without_msg_type() {
        let raw = b"8=FIX.4.4\x0134=1\x01";
        assert_eq!(FixMessage::decode(raw).unwrap_err(), FixParseError::MissingMsgType);
    }
}
