//! T0 検証用の probe。
//!
//! Zellij から届く attention / program status 関連のイベントを記録し、
//! 画面表示とログファイル（`/host/target/t0-probe.log`）の両方に出す。
//! 本実装（doc/tasks.md のフェーズ1）が揃ったら削除する前提。
//!
//! `ProgramStatusRecord` などの型は zellij-tile の prelude から再エクスポートされていないため、
//! 型名を書かずにフィールドとメソッド（`as_str` など）だけで扱っている。

use std::collections::{BTreeMap, VecDeque};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use zellij_tile::prelude::*;

const LOG_PATH: &str = "/host/target/t0-probe.log";
const MAX_RECENT: usize = 200;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Default)]
pub struct Probe {
    pub permission: Option<PermissionStatus>,
    counts: BTreeMap<&'static str, u64>,
    /// PaneUpdate の受信間隔（直近・最小）
    last_pane_update_ms: Option<u64>,
    last_interval_ms: Option<u64>,
    min_interval_ms: Option<u64>,
    /// PaneUpdate のうち、attention / program_status が何も変わらなかった回数
    pane_update_noop: u64,
    /// pane id -> 比較用スナップショット文字列
    snapshots: BTreeMap<u32, String>,
    /// 現在の表示用（ターミナルペインのみ）
    panes: Vec<PaneView>,
    recent: VecDeque<String>,
    log_error: Option<String>,
    start_ms: u64,
    /// Timer の発火回数
    ticks: u64,
    /// 前回の集計ログ時点のイベント数（PaneUpdate, noop, TabUpdate, PaneAttentionChanged）
    window_base: (u64, u64, u64, u64),
    /// get_pane_info のポーリング結果（pane id -> program_status の Debug 表現）
    poll_snapshots: BTreeMap<u32, String>,
}

/// get_pane_info をポーリングする間隔
pub const TICK_SECS: f64 = 0.5;
/// この回数の tick ごとにイベント数の集計をログに出す（0.5s * 10 = 5s）
const WINDOW_TICKS: u64 = 10;

struct PaneView {
    tab: usize,
    id: u32,
    title: String,
    lines: Vec<String>,
}

impl Probe {
    pub fn new() -> Self {
        let mut probe = Probe {
            start_ms: now_ms(),
            ..Default::default()
        };
        probe.log("==== probe loaded ====");
        probe
    }

    fn bump(&mut self, name: &'static str) {
        *self.counts.entry(name).or_default() += 1;
    }

    fn count(&self, name: &'static str) -> u64 {
        self.counts.get(name).copied().unwrap_or(0)
    }

    /// イベント数の定期集計と、get_pane_info のポーリングを行う。
    /// イベントで届かない更新（id 付きレコードなど）が実際には反映されているかを見るため。
    fn on_tick(&mut self) -> bool {
        self.ticks += 1;
        let mut logged = false;

        if self.ticks.is_multiple_of(WINDOW_TICKS) {
            let now = (
                self.count("PaneUpdate"),
                self.pane_update_noop,
                self.count("TabUpdate"),
                self.count("PaneAttentionChanged"),
            );
            let base = self.window_base;
            self.log(format!(
                "WINDOW {}s: PaneUpdate +{} (noop +{}) TabUpdate +{} PaneAttentionChanged +{}",
                TICK_SECS * WINDOW_TICKS as f64,
                now.0 - base.0,
                now.1 - base.1,
                now.2 - base.2,
                now.3 - base.3,
            ));
            self.window_base = now;
            logged = true;
        }

        let ids: Vec<u32> = self.snapshots.keys().copied().collect();
        for id in ids {
            let snapshot = match get_pane_info(PaneId::Terminal(id)) {
                Some(info) => format!(
                    "attention={}:{:?} program_status={:?}",
                    info.attention.status, info.attention.text, info.program_status
                ),
                None => "not found".to_owned(),
            };
            if self.poll_snapshots.get(&id) != Some(&snapshot) {
                self.log(format!("POLL terminal_{} {}", id, snapshot));
                self.poll_snapshots.insert(id, snapshot);
                logged = true;
            }
        }

        set_timeout(TICK_SECS);
        logged
    }

    /// 起動からの経過時間付きで、画面の履歴とログファイルに 1 行追加する。
    fn log(&mut self, line: impl AsRef<str>) {
        let line = format!(
            "[+{:>7}ms] {}",
            now_ms().saturating_sub(self.start_ms),
            line.as_ref()
        );
        if self.recent.len() >= MAX_RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(line.clone());
        let result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOG_PATH)
            .and_then(|mut f| writeln!(f, "{}", line));
        if let Err(e) = result {
            self.log_error = Some(e.to_string());
        }
    }

    pub fn on_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                self.bump("PermissionRequestResult");
                self.permission = Some(*status);
                self.log(format!("PermissionRequestResult {:?}", status));
                true
            }
            Event::PaneUpdate(manifest) => {
                self.bump("PaneUpdate");
                self.on_pane_update(manifest);
                true
            }
            Event::TabUpdate(tabs) => {
                self.bump("TabUpdate");
                let tabs: Vec<String> = tabs
                    .iter()
                    .filter(|t| !t.attention.status.is_idle())
                    .map(|t| format!("{}:{:?}", t.name, t.attention))
                    .collect();
                if !tabs.is_empty() {
                    self.log(format!("TabUpdate non-idle tabs: {}", tabs.join(" / ")));
                }
                true
            }
            Event::PaneAttentionChanged(pane_id, attention) => {
                self.bump("PaneAttentionChanged");
                self.log(format!(
                    "PaneAttentionChanged {:?} {:?}",
                    pane_id, attention
                ));
                true
            }
            Event::GlobalAttentionUpdate(global) => {
                self.bump("GlobalAttentionUpdate");
                let items: Vec<String> = global
                    .items
                    .iter()
                    .map(|i| {
                        format!(
                            "{}/{:?}={}:{}",
                            i.session,
                            i.item.pane_id,
                            i.item.attention.status,
                            i.text()
                        )
                    })
                    .collect();
                self.log(format!(
                    "GlobalAttentionUpdate status={} counts={:?} items=[{}]",
                    global.status,
                    global.counts,
                    items.join(", ")
                ));
                true
            }
            Event::Timer(_) => self.on_tick(),
            _ => false,
        }
    }

    fn on_pane_update(&mut self, manifest: &PaneManifest) {
        let now = now_ms();
        if let Some(prev) = self.last_pane_update_ms {
            let interval = now.saturating_sub(prev);
            self.last_interval_ms = Some(interval);
            self.min_interval_ms = Some(self.min_interval_ms.map_or(interval, |m| m.min(interval)));
        }
        self.last_pane_update_ms = Some(now);

        let mut panes = vec![];
        let mut seen = vec![];
        let mut changed = false;
        let mut tabs: Vec<_> = manifest.panes.iter().collect();
        tabs.sort_by_key(|(tab, _)| **tab);
        for (tab, infos) in tabs {
            for info in infos.iter().filter(|p| !p.is_plugin) {
                seen.push(info.id);
                let mut lines = vec![format!("  attention: {}", fmt_attention(&info.attention))];
                for (i, r) in info.program_status.iter().enumerate() {
                    lines.push(format!(
                        "  ps[{}]: id={} state={} kind={} progress={} app={} title={} msg={} updated_ms={}",
                        i,
                        opt(&r.id),
                        r.state.map_or("-", |s| s.as_str()),
                        r.kind.map_or("-", |k| k.as_str()),
                        r.progress.map_or("-".to_owned(), |p| p.to_string()),
                        opt(&r.app),
                        opt(&r.title),
                        opt(&r.msg),
                        r.updated_ms,
                    ));
                }
                // 変化検出は Debug 表現の比較で行う（型名を書かずに済ませるため）
                let snapshot = format!("{:?}|{:?}", info.attention, info.program_status);
                if self.snapshots.get(&info.id) != Some(&snapshot) {
                    changed = true;
                    self.log(format!(
                        "PaneUpdate tab={} terminal_{} title={:?}\n    attention={:?}\n    program_status={:?}",
                        tab, info.id, info.title, info.attention, info.program_status
                    ));
                    self.snapshots.insert(info.id, snapshot);
                }
                panes.push(PaneView {
                    tab: *tab,
                    id: info.id,
                    title: info.title.clone(),
                    lines,
                });
            }
        }
        let closed: Vec<u32> = self
            .snapshots
            .keys()
            .filter(|id| !seen.contains(id))
            .copied()
            .collect();
        for id in closed {
            changed = true;
            self.snapshots.remove(&id);
            self.log(format!("PaneUpdate terminal_{} disappeared", id));
        }
        if !changed {
            self.pane_update_noop += 1;
        }
        self.panes = panes;
    }

    pub fn render(&self, rows: usize, cols: usize) {
        let mut out = vec![];
        let counts: Vec<String> = self
            .counts
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        out.push(format!(
            "[T0 probe] permission={:?} | {}",
            self.permission,
            counts.join(" ")
        ));
        out.push(format!(
            "PaneUpdate interval last={:?}ms min={:?}ms noop={} | log={} {}",
            self.last_interval_ms,
            self.min_interval_ms,
            self.pane_update_noop,
            LOG_PATH,
            self.log_error
                .as_ref()
                .map_or("(ok)".to_owned(), |e| format!("(write error: {})", e)),
        ));
        out.push("--- terminal panes ---".to_owned());
        for p in &self.panes {
            out.push(format!("tab{} terminal_{} {:?}", p.tab, p.id, p.title));
            out.extend(p.lines.iter().cloned());
        }
        out.push("--- recent events (newest last) ---".to_owned());
        let remaining = rows.saturating_sub(out.len());
        // 複数行のログは 1 行ずつに分けてから末尾を表示する
        let recent: Vec<&str> = self.recent.iter().flat_map(|l| l.lines()).collect();
        let skip = recent.len().saturating_sub(remaining);
        out.extend(recent[skip..].iter().map(|l| l.to_string()));

        for (i, line) in out.iter().take(rows).enumerate() {
            let line: String = line.chars().take(cols).collect();
            if i + 1 < rows {
                println!("{}", line);
            } else {
                print!("{}", line);
            }
        }
    }
}

fn opt(v: &Option<String>) -> String {
    v.as_ref().map_or("-".to_owned(), |s| format!("{:?}", s))
}

fn fmt_attention(a: &PaneAttention) -> String {
    format!(
        "status={} source={} text={:?} detail(kind={} progress={:?} app={:?} title={:?} body={:?}) seen={} muted={:?} since_ms={} changed_ms={}",
        a.status,
        a.source.map_or("-", |s| s.as_str()),
        a.text,
        a.detail.kind.map_or("-", |k| k.as_str()),
        a.detail.progress,
        a.detail.app,
        a.detail.title,
        a.detail.body,
        a.seen,
        a.muted,
        a.since_ms,
        a.changed_ms,
    )
}
