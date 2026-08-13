use crate::logger::LogLevel;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BreakKind {
    Short,
    Long,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BreakTemplate {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DisableOption {
    pub label: String,
    pub time: u64,
    pub unit: String,
}

impl DisableOption {
    #[cfg(test)]
    pub fn seconds(&self) -> u64 {
        match self.unit.as_str() {
            "hour" => self.time.saturating_mul(60 * 60),
            _ => self.time.saturating_mul(60),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostponeOption {
    pub duration: u64,
    pub unit: String,
    #[serde(default, skip_deserializing)]
    pub seconds: u64,
}

fn default_skip_limit() -> u8 {
    2
}

fn default_postpone_duration() -> u64 {
    5
}

fn default_postpone_unit() -> String {
    "minutes".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakEngineConfig {
    pub break_interval: u64,
    pub long_break_duration: u64,
    pub no_of_short_breaks_per_long_break: u8,
    pub pre_break_warning_time: u64,
    pub short_break_duration: u64,
    pub persist_state: bool,
    pub random_order: bool,
    pub allow_postpone: bool,
    pub postpone_duration_seconds: u64,
    pub postpone_options: Vec<PostponeOption>,
    pub strict_break: bool,
    pub consecutive_skip_limit: u8,
    pub idle_time: u64,
    pub log_level: LogLevel,
    pub short_breaks: Vec<BreakTemplate>,
    pub long_breaks: Vec<BreakTemplate>,
    pub disable_options: Vec<DisableOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBreakEngineConfig {
    #[serde(default)]
    meta: Option<RawConfigMeta>,
    #[serde(default)]
    log_level: Option<String>,
    #[serde(default)]
    random_order: bool,
    #[serde(default)]
    allow_postpone: bool,
    #[serde(default)]
    persist_state: bool,
    #[serde(default = "default_postpone_duration")]
    postpone_duration: u64,
    #[serde(default = "default_postpone_unit")]
    postpone_unit: String,
    #[serde(default)]
    postpone_options: Vec<PostponeOption>,
    short_break_interval: u64,
    long_break_interval: u64,
    long_break_duration: u64,
    pre_break_warning_time: u64,
    short_break_duration: u64,
    strict_break: bool,
    #[serde(default = "default_skip_limit")]
    consecutive_skip_limit: u8,
    #[serde(default)]
    idle_time: u64,
    #[serde(default)]
    short_breaks: Vec<BreakTemplate>,
    #[serde(default)]
    long_breaks: Vec<BreakTemplate>,
    #[serde(default)]
    disable_options: Vec<DisableOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
struct RawConfigMeta {
    #[serde(default)]
    config_version: Option<String>,
}

impl BreakEngineConfig {
    pub fn load() -> Self {
        Self::load_from_embedded_defaults().expect("defaults config should be valid YAML")
    }

    pub fn load_from_embedded_defaults() -> Result<Self, serde_yaml::Error> {
        Self::from_yaml(include_str!("../config/defaults.yaml"))
    }

    pub fn load_or_create_from_path(path: &Path, default_yaml: &str) -> Result<Self, String> {
        let path = crate::config_file::ensure_config_file(path, default_yaml)?;
        let yaml = fs::read_to_string(path).map_err(|error| error.to_string())?;
        Self::from_yaml(&yaml).map_err(|error| error.to_string())
    }

    fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        let raw: RawBreakEngineConfig = serde_yaml::from_str(yaml)?;
        Ok(Self::from_raw(raw))
    }

    fn from_raw(raw: RawBreakEngineConfig) -> Self {
        let breaks_per_long = if raw.short_break_interval == 0 {
            0
        } else {
            raw.long_break_interval
                .saturating_div(raw.short_break_interval)
                .saturating_sub(1)
                .min(u8::MAX as u64) as u8
        };

        Self {
            break_interval: raw.short_break_interval,
            long_break_duration: raw.long_break_duration,
            no_of_short_breaks_per_long_break: breaks_per_long,
            pre_break_warning_time: raw.pre_break_warning_time,
            short_break_duration: raw.short_break_duration,
            persist_state: raw.persist_state,
            random_order: raw.random_order,
            allow_postpone: raw.allow_postpone,
            postpone_duration_seconds: postpone_seconds(raw.postpone_duration, &raw.postpone_unit),
            postpone_options: raw
                .postpone_options
                .into_iter()
                .map(|option| PostponeOption {
                    seconds: postpone_seconds(option.duration, &option.unit),
                    ..option
                })
                .collect(),
            strict_break: raw.strict_break,
            consecutive_skip_limit: raw.consecutive_skip_limit,
            idle_time: raw.idle_time,
            log_level: LogLevel::parse(raw.log_level.as_deref()),
            short_breaks: raw.short_breaks,
            long_breaks: raw.long_breaks,
            disable_options: raw.disable_options,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnginePhase {
    Stopped,
    Running,
    Warning,
    OnBreak,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakInfo {
    pub kind: BreakKind,
    pub duration_seconds: u64,
    pub mandatory: bool,
    pub template_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineStatus {
    pub phase: EnginePhase,
    pub seconds_remaining: Option<u64>,
    pub break_interval_minutes: u64,
    pub warning_seconds: u64,
    pub upcoming_break_kind: Option<BreakKind>,
    pub postpone_reason: Option<String>,
    pub current_break: Option<BreakInfo>,
    pub can_skip: bool,
    pub can_postpone: bool,
    pub skip_limit_reached: bool,
    pub disable_options: Vec<DisableOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakEngineSnapshot {
    pub was_started: bool,
    pub phase: EnginePhase,
    pub work_remaining: u64,
    pub warning_remaining: u64,
    pub break_remaining: u64,
    pub disabled_remaining: u64,
    pub shorts_since_long: u8,
    pub next_short_index: usize,
    pub next_long_index: usize,
    pub short_break_order: Vec<usize>,
    pub long_break_order: Vec<usize>,
    pub current_break: Option<BreakInfo>,
    pub idle_active: bool,
    pub idle_elapsed_seconds: u64,
    pub fullscreen: bool,
    pub consecutive_skips: u8,
    pub saved_at_unix_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct BreakEngine {
    config: BreakEngineConfig,
    phase: EnginePhase,
    work_remaining: u64,
    warning_remaining: u64,
    break_remaining: u64,
    disabled_remaining: u64,
    shorts_since_long: u8,
    next_short_index: usize,
    next_long_index: usize,
    short_break_order: Vec<usize>,
    long_break_order: Vec<usize>,
    current_break: Option<BreakInfo>,
    idle_active: bool,
    idle_elapsed_seconds: u64,
    fullscreen: bool,
    last_synced_at: Option<Instant>,
    consecutive_skips: u8,
    shuffle_rng: fastrand::Rng,
}

impl BreakEngine {
    pub fn new(config: BreakEngineConfig) -> Self {
        let interval = config.break_interval.saturating_mul(60);
        let mut engine = Self {
            config,
            phase: EnginePhase::Stopped,
            work_remaining: interval,
            warning_remaining: 0,
            break_remaining: 0,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: Vec::new(),
            long_break_order: Vec::new(),
            current_break: None,
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            last_synced_at: None,
            consecutive_skips: 0,
            shuffle_rng: fastrand::Rng::new(),
        };
        engine.reset_template_orders();
        engine
    }

    pub fn start(&mut self) -> EngineStatus {
        self.phase = EnginePhase::Running;
        self.work_remaining = self.config.break_interval.saturating_mul(60);
        self.warning_remaining = 0;
        self.break_remaining = 0;
        self.disabled_remaining = 0;
        self.current_break = None;
        self.idle_elapsed_seconds = 0;
        self.consecutive_skips = 0;
        self.last_synced_at = Some(Instant::now());
        self.reconcile();
        self.status()
    }

    pub fn stop(&mut self) -> EngineStatus {
        self.phase = EnginePhase::Stopped;
        self.work_remaining = self.config.break_interval.saturating_mul(60);
        self.warning_remaining = 0;
        self.break_remaining = 0;
        self.disabled_remaining = 0;
        self.current_break = None;
        self.idle_elapsed_seconds = 0;
        self.consecutive_skips = 0;
        self.last_synced_at = None;
        self.status()
    }

    #[cfg(test)]
    pub fn advance_by(&mut self, seconds: u64) -> EngineStatus {
        self.advance_by_seconds(seconds);
        self.status()
    }

    #[cfg(test)]
    pub fn tick(&mut self, seconds: u64) {
        self.advance_by_seconds(seconds);
    }

    pub fn status(&mut self) -> EngineStatus {
        self.sync_with_clock();
        self.reconcile();
        self.build_status()
    }

    pub fn restore_elapsed(&mut self, elapsed_seconds: u64) -> EngineStatus {
        self.advance_by_seconds(elapsed_seconds);
        self.last_synced_at = Some(Instant::now());
        self.reconcile();
        self.build_status()
    }

    fn build_status(&self) -> EngineStatus {
        let skip_allowed = !self.config.strict_break
            && self.consecutive_skips < self.config.consecutive_skip_limit;
        let skip_limit_reached = self.consecutive_skips >= self.config.consecutive_skip_limit;
        EngineStatus {
            phase: self.phase.clone(),
            seconds_remaining: match self.phase {
                EnginePhase::Stopped => None,
                EnginePhase::Running => Some(self.work_remaining),
                EnginePhase::Warning => Some(self.warning_remaining),
                EnginePhase::OnBreak => Some(self.break_remaining),
                EnginePhase::Disabled => Some(self.disabled_remaining),
            },
            break_interval_minutes: self.config.break_interval,
            warning_seconds: self.config.pre_break_warning_time,
            upcoming_break_kind: self.upcoming_break_kind(),
            postpone_reason: self.postpone_reason().map(str::to_string),
            current_break: self.current_break.clone(),
            can_skip: self.current_break.is_some() && skip_allowed,
            can_postpone: self.current_break.is_some() && self.config.allow_postpone,
            skip_limit_reached,
            disable_options: self.config.disable_options.clone(),
        }
    }

    pub fn current_break(&self) -> Option<BreakInfo> {
        self.current_break.clone()
    }

    pub fn begin_break_now(&mut self) -> BreakInfo {
        self.sync_with_clock();
        if self.current_break.is_none() {
            self.start_break();
        }
        self.current_break
            .clone()
            .expect("begin_break_now should return an active break")
    }

    pub fn config(&self) -> &BreakEngineConfig {
        &self.config
    }

    pub fn apply_config(&mut self, config: BreakEngineConfig) {
        self.sync_with_clock();
        self.config = config;
        self.reconcile_phase_after_config_change();
    }

    pub fn snapshot(&mut self, saved_at_unix_seconds: u64) -> BreakEngineSnapshot {
        self.sync_with_clock();
        self.reconcile();
        BreakEngineSnapshot {
            was_started: !matches!(self.phase, EnginePhase::Stopped),
            phase: self.phase.clone(),
            work_remaining: self.work_remaining,
            warning_remaining: self.warning_remaining,
            break_remaining: self.break_remaining,
            disabled_remaining: self.disabled_remaining,
            shorts_since_long: self.shorts_since_long,
            next_short_index: self.next_short_index,
            next_long_index: self.next_long_index,
            short_break_order: self.short_break_order.clone(),
            long_break_order: self.long_break_order.clone(),
            current_break: self.current_break.clone(),
            idle_active: self.idle_active,
            idle_elapsed_seconds: self.idle_elapsed_seconds,
            fullscreen: self.fullscreen,
            consecutive_skips: self.consecutive_skips,
            saved_at_unix_seconds,
        }
    }

    pub fn from_snapshot(config: BreakEngineConfig, snapshot: BreakEngineSnapshot) -> Self {
        let mut engine = Self::new(config);
        let was_started = snapshot.was_started;
        engine.phase = if was_started {
            snapshot.phase
        } else {
            EnginePhase::Stopped
        };
        engine.work_remaining = snapshot.work_remaining;
        engine.warning_remaining = snapshot.warning_remaining;
        engine.break_remaining = snapshot.break_remaining;
        engine.disabled_remaining = snapshot.disabled_remaining;
        engine.shorts_since_long = snapshot.shorts_since_long;
        engine.next_short_index = snapshot.next_short_index;
        engine.next_long_index = snapshot.next_long_index;
        engine.short_break_order = snapshot.short_break_order;
        engine.long_break_order = snapshot.long_break_order;
        engine.current_break = snapshot.current_break;
        engine.idle_active = snapshot.idle_active;
        engine.idle_elapsed_seconds = snapshot.idle_elapsed_seconds;
        engine.fullscreen = snapshot.fullscreen;
        engine.consecutive_skips = snapshot.consecutive_skips;
        engine.normalize_snapshot_template_orders();
        engine.normalize_snapshot_cycle_counter();
        if was_started {
            engine.normalize_imported_started_state();
        } else {
            engine.phase = EnginePhase::Stopped;
            engine.work_remaining = engine.config.break_interval.saturating_mul(60);
            engine.warning_remaining = 0;
            engine.break_remaining = 0;
            engine.disabled_remaining = 0;
            engine.shorts_since_long = 0;
            engine.next_short_index = 0;
            engine.next_long_index = 0;
            engine.current_break = None;
            engine.idle_active = false;
            engine.idle_elapsed_seconds = 0;
            engine.fullscreen = false;
            engine.consecutive_skips = 0;
            engine.reset_template_orders();
        }
        engine.last_synced_at = if was_started {
            Some(Instant::now())
        } else {
            None
        };
        engine
    }

    pub fn set_idle(&mut self, idle: bool) {
        self.sync_with_clock();
        if idle && !self.idle_active {
            self.idle_elapsed_seconds = 0;
        }
        if !idle {
            self.idle_elapsed_seconds = 0;
        }
        self.idle_active = idle;
        self.reconcile();
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.sync_with_clock();
        self.fullscreen = fullscreen;
        self.reconcile();
    }

    pub fn disable_for(&mut self, seconds: u64) -> Result<EngineStatus, String> {
        self.sync_with_clock();
        if matches!(self.phase, EnginePhase::OnBreak) {
            return Err("Cannot disable reminders during an active break.".into());
        }
        self.phase = EnginePhase::Disabled;
        self.disabled_remaining = seconds;
        self.last_synced_at = Some(Instant::now());
        Ok(self.status())
    }

    pub fn skip_break(&mut self) -> Result<EngineStatus, String> {
        self.sync_with_clock();
        if !matches!(self.phase, EnginePhase::OnBreak) {
            return Err("No active break to skip.".into());
        }
        if self.config.strict_break {
            return Err("This break is mandatory; skipping is disabled.".into());
        }
        if self.consecutive_skips >= self.config.consecutive_skip_limit {
            return Err("Skip limit reached".into());
        }
        self.consecutive_skips = self.consecutive_skips.saturating_add(1);
        self.finish_break_cycle();
        Ok(self.status())
    }

    pub fn complete_break(&mut self) -> Result<EngineStatus, String> {
        self.sync_with_clock();
        if !matches!(self.phase, EnginePhase::OnBreak) {
            return Ok(self.status());
        }
        self.finish_break_cycle();
        self.consecutive_skips = 0;
        Ok(self.status())
    }

    pub fn postpone_break(&mut self) -> Result<EngineStatus, String> {
        self.postpone_break_with_override(None)
    }

    pub fn postpone_break_with_override(
        &mut self,
        override_seconds: Option<u64>,
    ) -> Result<EngineStatus, String> {
        self.sync_with_clock();
        if !matches!(self.phase, EnginePhase::OnBreak) {
            return Err("No active break to postpone.".into());
        }
        if !self.config.allow_postpone {
            return Err("Postpone is disabled.".into());
        }
        let postpone_seconds = match override_seconds {
            Some(seconds) => {
                if !self.config.postpone_options.is_empty()
                    && !self
                        .config
                        .postpone_options
                        .iter()
                        .any(|option| option.seconds == seconds)
                {
                    return Err("Selected postpone duration is not configured.".into());
                }
                seconds
            }
            None => self.config.postpone_duration_seconds,
        };

        self.phase = EnginePhase::Running;
        self.current_break = None;
        self.break_remaining = 0;
        self.warning_remaining = 0;
        self.work_remaining = postpone_seconds;
        self.last_synced_at = Some(Instant::now());
        self.reconcile();
        Ok(self.status())
    }

    #[cfg(test)]
    pub fn debug_force_break(&mut self) -> BreakInfo {
        self.start_break();
        self.current_break
            .clone()
            .expect("debug force break should create a break")
    }

    #[cfg(test)]
    pub fn rewind_last_sync_by(&mut self, seconds: u64) {
        if let Some(last_synced_at) = self.last_synced_at {
            self.last_synced_at = Some(last_synced_at - Duration::from_secs(seconds));
        }
    }

    #[cfg(test)]
    pub fn set_random_seed_for_tests(&mut self, seed: u64) {
        self.shuffle_rng = fastrand::Rng::with_seed(seed);
        self.reset_template_orders();
    }

    fn reconcile(&mut self) {
        match self.phase {
            EnginePhase::Stopped | EnginePhase::Disabled | EnginePhase::OnBreak => {}
            EnginePhase::Running => {
                let warning_time = self.config.pre_break_warning_time.min(self.work_remaining);
                if self.postpone_reason().is_none() && self.work_remaining == warning_time {
                    if warning_time > 0 {
                        self.phase = EnginePhase::Warning;
                        self.warning_remaining = warning_time;
                    } else {
                        self.start_break();
                    }
                }
            }
            EnginePhase::Warning => {
                if self.warning_remaining == 0 && self.postpone_reason().is_none() {
                    self.start_break();
                }
            }
        }
    }

    fn sync_with_clock(&mut self) {
        let Some(last_synced_at) = self.last_synced_at else {
            return;
        };

        let elapsed = last_synced_at.elapsed().as_secs();
        if elapsed == 0 {
            return;
        }

        self.advance_by_seconds(elapsed);
        self.last_synced_at = Some(last_synced_at + Duration::from_secs(elapsed));
    }

    fn postpone_reason(&self) -> Option<&'static str> {
        let idle_threshold_seconds = self.config.idle_time.saturating_mul(60);
        if self.idle_active && self.idle_elapsed_seconds >= idle_threshold_seconds {
            Some("idle")
        } else if self.fullscreen {
            Some("fullscreen")
        } else {
            None
        }
    }

    fn upcoming_break_kind(&self) -> Option<BreakKind> {
        match self.phase {
            EnginePhase::Stopped => None,
            EnginePhase::OnBreak => self.current_break.as_ref().map(|info| info.kind),
            EnginePhase::Running | EnginePhase::Warning | EnginePhase::Disabled => {
                if self.shorts_since_long >= self.config.no_of_short_breaks_per_long_break {
                    Some(BreakKind::Long)
                } else {
                    Some(BreakKind::Short)
                }
            }
        }
    }

    fn start_break(&mut self) {
        let (kind, duration_seconds, template_name) =
            if self.shorts_since_long >= self.config.no_of_short_breaks_per_long_break {
                self.shorts_since_long = 0;
                (
                    BreakKind::Long,
                    self.config.long_break_duration,
                    self.next_template_name(BreakKind::Long),
                )
            } else {
                self.shorts_since_long = self.shorts_since_long.saturating_add(1);
                (
                    BreakKind::Short,
                    self.config.short_break_duration,
                    self.next_template_name(BreakKind::Short),
                )
            };

        self.phase = EnginePhase::OnBreak;
        self.break_remaining = duration_seconds;
        self.warning_remaining = 0;
        self.current_break = Some(BreakInfo {
            kind,
            duration_seconds,
            mandatory: self.config.strict_break,
            template_name,
        });
        self.last_synced_at = Some(Instant::now());
    }

    fn finish_break_cycle(&mut self) {
        self.phase = EnginePhase::Running;
        self.current_break = None;
        self.break_remaining = 0;
        self.warning_remaining = 0;
        self.work_remaining = self.config.break_interval.saturating_mul(60);
        self.last_synced_at = Some(Instant::now());
        self.reconcile();
    }

    fn advance_by_seconds(&mut self, mut seconds: u64) {
        while seconds > 0 {
            match self.phase {
                EnginePhase::Stopped => break,
                EnginePhase::Disabled => {
                    let step = seconds.min(self.disabled_remaining);
                    self.advance_idle_elapsed(step);
                    self.disabled_remaining = self.disabled_remaining.saturating_sub(step);
                    seconds -= step;
                    if self.disabled_remaining == 0 {
                        self.phase = EnginePhase::Running;
                        self.reconcile();
                    }
                }
                EnginePhase::Running => {
                    let warning_time = self.config.pre_break_warning_time.min(self.work_remaining);
                    let to_warning = self.work_remaining.saturating_sub(warning_time);
                    if self.postpone_reason().is_some() {
                        if to_warning == 0 {
                            break;
                        }
                        let step = seconds.min(to_warning);
                        self.advance_idle_elapsed(step);
                        self.work_remaining = self.work_remaining.saturating_sub(step);
                        seconds -= step;
                        if self.work_remaining == warning_time {
                            self.reconcile();
                            break;
                        }
                    } else if to_warning == 0 {
                        self.reconcile();
                    } else {
                        let step = seconds.min(to_warning);
                        self.advance_idle_elapsed(step);
                        self.work_remaining = self.work_remaining.saturating_sub(step);
                        seconds -= step;
                        if self.work_remaining == warning_time {
                            self.reconcile();
                        }
                    }
                }
                EnginePhase::Warning => {
                    if self.postpone_reason().is_some() {
                        break;
                    }
                    let step = seconds.min(self.warning_remaining);
                    self.advance_idle_elapsed(step);
                    self.warning_remaining = self.warning_remaining.saturating_sub(step);
                    seconds -= step;
                    if self.warning_remaining == 0 {
                        self.start_break();
                    }
                }
                EnginePhase::OnBreak => {
                    let step = seconds.min(self.break_remaining);
                    self.advance_idle_elapsed(step);
                    self.break_remaining = self.break_remaining.saturating_sub(step);
                    seconds -= step;
                    if self.break_remaining == 0 {
                        self.finish_break_cycle();
                        self.consecutive_skips = 0;
                    }
                }
            }
        }
    }

    fn advance_idle_elapsed(&mut self, step: u64) {
        if self.idle_active {
            self.idle_elapsed_seconds = self.idle_elapsed_seconds.saturating_add(step);
        }
    }

    fn reconcile_phase_after_config_change(&mut self) {
        self.normalize_snapshot_template_orders();
        self.normalize_snapshot_cycle_counter();

        if matches!(self.phase, EnginePhase::Stopped) {
            return;
        }

        self.warning_remaining = self
            .warning_remaining
            .min(self.config.pre_break_warning_time);

        if let Some(current_break) = self.current_break.as_mut() {
            let replacement_duration = match current_break.kind {
                BreakKind::Short => self.config.short_break_duration,
                BreakKind::Long => self.config.long_break_duration,
            };
            current_break.duration_seconds = replacement_duration;
            current_break.mandatory = self.config.strict_break;
            self.break_remaining = self.break_remaining.min(replacement_duration);
        }
    }

    fn next_template_name(&mut self, kind: BreakKind) -> Option<String> {
        match kind {
            BreakKind::Short => {
                if self.config.short_breaks.is_empty() {
                    None
                } else {
                    let index = self.current_template_index(BreakKind::Short)?;
                    let template = self.config.short_breaks[index].name.clone();
                    Some(template)
                }
            }
            BreakKind::Long => {
                if self.config.long_breaks.is_empty() {
                    None
                } else {
                    let index = self.current_template_index(BreakKind::Long)?;
                    let template = self.config.long_breaks[index].name.clone();
                    Some(template)
                }
            }
        }
    }

    fn current_template_index(&mut self, kind: BreakKind) -> Option<usize> {
        match kind {
            BreakKind::Short => {
                if self.short_break_order.is_empty() {
                    return None;
                }
                let index = self.short_break_order[self.next_short_index];
                self.next_short_index += 1;
                if self.next_short_index >= self.short_break_order.len() {
                    self.next_short_index = 0;
                    if self.config.random_order {
                        self.shuffle_rng.shuffle(&mut self.short_break_order);
                    }
                }
                Some(index)
            }
            BreakKind::Long => {
                if self.long_break_order.is_empty() {
                    return None;
                }
                let index = self.long_break_order[self.next_long_index];
                self.next_long_index += 1;
                if self.next_long_index >= self.long_break_order.len() {
                    self.next_long_index = 0;
                    if self.config.random_order {
                        self.shuffle_rng.shuffle(&mut self.long_break_order);
                    }
                }
                Some(index)
            }
        }
    }

    fn reset_template_orders(&mut self) {
        self.short_break_order = (0..self.config.short_breaks.len()).collect();
        self.long_break_order = (0..self.config.long_breaks.len()).collect();
        if self.config.random_order {
            self.shuffle_rng.shuffle(&mut self.short_break_order);
            self.shuffle_rng.shuffle(&mut self.long_break_order);
        }
        self.next_short_index = 0;
        self.next_long_index = 0;
    }

    fn normalize_snapshot_template_orders(&mut self) {
        Self::normalize_template_order(
            &mut self.short_break_order,
            &mut self.next_short_index,
            self.config.short_breaks.len(),
        );
        Self::normalize_template_order(
            &mut self.long_break_order,
            &mut self.next_long_index,
            self.config.long_breaks.len(),
        );
    }

    fn normalize_template_order(
        order: &mut Vec<usize>,
        next_index: &mut usize,
        expected_len: usize,
    ) {
        let valid =
            order.len() == expected_len && order.iter().all(|&index| index < expected_len) && {
                let mut seen = vec![false; expected_len];
                order.iter().all(|&index| {
                    if seen[index] {
                        false
                    } else {
                        seen[index] = true;
                        true
                    }
                })
            };

        if !valid {
            *order = (0..expected_len).collect();
            *next_index = 0;
            return;
        }

        if *next_index >= order.len() {
            *next_index = 0;
        }
    }

    fn normalize_imported_started_state(&mut self) {
        if matches!(self.phase, EnginePhase::OnBreak) && self.current_break.is_none() {
            self.phase = EnginePhase::Running;
            self.reset_imported_running_state();
        }

        if matches!(self.phase, EnginePhase::OnBreak) && self.break_remaining == 0 {
            self.phase = EnginePhase::Running;
            self.reset_imported_running_state();
        }

        if matches!(self.phase, EnginePhase::Disabled) && self.disabled_remaining == 0 {
            self.phase = EnginePhase::Running;
            self.reset_imported_running_state();
        }

        if matches!(self.phase, EnginePhase::Running) {
            self.clear_imported_running_state();
            if self.work_remaining == 0 {
                self.work_remaining = self.config.break_interval.saturating_mul(60);
            }
        }

        if matches!(self.phase, EnginePhase::Warning) && self.warning_remaining == 0 {
            self.phase = EnginePhase::Running;
            self.reset_imported_running_state();
        }

        if matches!(self.phase, EnginePhase::Warning) {
            self.clear_imported_warning_state();
        }

        if matches!(self.phase, EnginePhase::Disabled) {
            self.clear_imported_disabled_state();
        }

        if matches!(self.phase, EnginePhase::OnBreak) {
            self.sanitize_imported_break_payload();
        }
    }

    fn normalize_snapshot_cycle_counter(&mut self) {
        self.shorts_since_long = self
            .shorts_since_long
            .min(self.config.no_of_short_breaks_per_long_break);
    }

    fn clear_imported_running_state(&mut self) {
        self.current_break = None;
        self.break_remaining = 0;
        self.warning_remaining = 0;
        self.disabled_remaining = 0;
    }

    fn reset_imported_running_state(&mut self) {
        self.work_remaining = self.config.break_interval.saturating_mul(60);
        self.current_break = None;
        self.break_remaining = 0;
        self.warning_remaining = 0;
        self.disabled_remaining = 0;
    }

    fn clear_imported_warning_state(&mut self) {
        self.current_break = None;
        self.break_remaining = 0;
        self.disabled_remaining = 0;
    }

    fn clear_imported_disabled_state(&mut self) {
        self.current_break = None;
        self.break_remaining = 0;
        self.warning_remaining = 0;
    }

    fn sanitize_imported_break_payload(&mut self) {
        if let Some(current_break) = self.current_break.as_mut() {
            current_break.duration_seconds = self.break_remaining;
            current_break.mandatory = self.config.strict_break;
            current_break.kind = if self.shorts_since_long == 0
                || self.shorts_since_long >= self.config.no_of_short_breaks_per_long_break
            {
                BreakKind::Long
            } else {
                BreakKind::Short
            };
            current_break.template_name = None;
        }
    }
}

fn postpone_seconds(duration: u64, unit: &str) -> u64 {
    match unit {
        "hour" | "hours" => duration.saturating_mul(60 * 60),
        _ => duration.saturating_mul(60),
    }
}

#[cfg(test)]
mod tests {
    use super::{BreakEngine, BreakEngineConfig, BreakInfo, BreakKind, BreakTemplate, EnginePhase};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn path(&self) -> &Path {
            self.path.as_path()
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_test_dir(name: &str) -> TestDir {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gazeguard-break-engine-{name}-{suffix}"));
        fs::create_dir_all(&path).unwrap();
        TestDir { path }
    }

    fn restore_test_config() -> BreakEngineConfig {
        let mut config = BreakEngineConfig::load();
        config.break_interval = 2;
        config.pre_break_warning_time = 5;
        config.short_break_duration = 8;
        config.long_break_duration = 30;
        config.no_of_short_breaks_per_long_break = 99;
        config.random_order = false;
        config.allow_postpone = false;
        config.strict_break = false;
        config.short_breaks = vec![BreakTemplate {
            name: "Restore short".into(),
        }];
        config.long_breaks = vec![BreakTemplate {
            name: "Restore long".into(),
        }];
        config
    }

    fn restore_snapshot(
        phase: EnginePhase,
        work_remaining: u64,
        warning_remaining: u64,
        break_remaining: u64,
        disabled_remaining: u64,
    ) -> super::BreakEngineSnapshot {
        let is_on_break = matches!(phase, EnginePhase::OnBreak);
        let current_break = if is_on_break {
            Some(BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: break_remaining,
                mandatory: false,
                template_name: Some("Restore short".into()),
            })
        } else {
            None
        };

        super::BreakEngineSnapshot {
            was_started: true,
            phase,
            work_remaining,
            warning_remaining,
            break_remaining,
            disabled_remaining,
            shorts_since_long: if is_on_break { 1 } else { 0 },
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break,
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 0,
            saved_at_unix_seconds: 0,
        }
    }

    #[test]
    fn loads_yaml_defaults_shape() {
        let config = BreakEngineConfig::load_from_embedded_defaults().unwrap();

        assert_eq!(config.break_interval, 15);
        assert_eq!(config.pre_break_warning_time, 10);
        assert_eq!(config.short_break_duration, 15);
        assert_eq!(config.long_break_duration, 60);
        assert_eq!(config.no_of_short_breaks_per_long_break, 4);
        assert_eq!(config.idle_time, 5);
        assert_eq!(config.log_level, crate::logger::LogLevel::Off);
        assert!(!config.strict_break);
        assert!(config.allow_postpone);
        assert_eq!(config.postpone_duration_seconds, 5 * 60);
        assert_eq!(config.postpone_options.len(), 3);
        assert_eq!(config.postpone_options[0].seconds, 5 * 60);
        assert_eq!(config.postpone_options[1].seconds, 10 * 60);
        assert_eq!(config.postpone_options[2].seconds, 15 * 60);
        assert_eq!(config.short_breaks.len(), 7);
        assert_eq!(config.long_breaks.len(), 2);
        assert_eq!(config.disable_options.len(), 4);
        assert!(config.persist_state);
        assert_eq!(config.disable_options[0].seconds(), 30 * 60);
        assert_eq!(config.short_breaks[0].name, "Gently close your eyes");
        assert_eq!(config.long_breaks[0].name, "Walk for a while");
    }

    #[test]
    fn load_or_create_from_path_reads_seeded_yaml_file() {
        let temp = unique_test_dir("seeded-config");
        let config_path = temp.path().join("config.yaml");
        let yaml = "short_break_interval: 7\nlong_break_interval: 75\nlong_break_duration: 60\npre_break_warning_time: 10\nshort_break_duration: 15\nstrict_break: false\n";

        let config = BreakEngineConfig::load_or_create_from_path(&config_path, yaml).unwrap();

        assert_eq!(config.break_interval, 7);
        assert_eq!(fs::read_to_string(&config_path).unwrap(), yaml);
    }

    #[test]
    fn apply_config_updates_schedule_values_and_preserves_runtime_state() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        engine.tick(3);
        engine.begin_break_now();
        engine.set_idle(true);
        engine.set_fullscreen(true);

        let before = engine.snapshot(0);

        let mut updated = BreakEngineConfig::load();
        updated.break_interval = 9;
        updated.pre_break_warning_time = 12;
        updated.short_break_duration = 22;
        updated.long_break_duration = 70;
        updated.idle_time = 11;
        updated.random_order = true;
        updated.short_breaks = vec![BreakTemplate {
            name: "Reload short".into(),
        }];
        updated.long_breaks = vec![];

        engine.apply_config(updated.clone());

        let after = engine.snapshot(0);
        let status = engine.status();
        assert_eq!(status.phase, before.phase);
        assert_eq!(after.phase, before.phase);
        assert_eq!(after.work_remaining, before.work_remaining);
        assert_eq!(after.warning_remaining, before.warning_remaining);
        assert_eq!(after.break_remaining, before.break_remaining);
        assert_eq!(after.shorts_since_long, before.shorts_since_long);
        assert_eq!(after.next_short_index, 0);
        assert_eq!(after.next_long_index, 0);
        assert_eq!(after.short_break_order, vec![0]);
        assert!(after.long_break_order.is_empty());
        assert_eq!(
            after.current_break.as_ref().map(|info| info.kind),
            before.current_break.as_ref().map(|info| info.kind)
        );
        assert_eq!(
            after
                .current_break
                .as_ref()
                .map(|info| info.template_name.clone()),
            before
                .current_break
                .as_ref()
                .map(|info| info.template_name.clone())
        );
        assert_eq!(
            after
                .current_break
                .as_ref()
                .map(|info| info.duration_seconds),
            Some(22)
        );
        assert_eq!(
            after.current_break.as_ref().map(|info| info.mandatory),
            Some(false)
        );
        assert!(after.idle_active);
        assert!(after.fullscreen);
        assert_eq!(after.consecutive_skips, before.consecutive_skips);
        assert_eq!(engine.config().break_interval, 9);
        assert_eq!(engine.config().pre_break_warning_time, 12);
        assert_eq!(engine.config().short_break_duration, 22);
        assert_eq!(engine.config().long_break_duration, 70);
        assert_eq!(engine.config().idle_time, 11);
    }

    #[test]
    fn apply_config_syncs_elapsed_wall_clock_before_replacing_config() {
        let mut config = BreakEngineConfig::load();
        config.break_interval = 1;
        let mut engine = BreakEngine::new(config);
        engine.start();
        engine.advance_by(40);
        engine.rewind_last_sync_by(10);

        let mut updated = BreakEngineConfig::load();
        updated.break_interval = 9;
        updated.pre_break_warning_time = 12;

        engine.apply_config(updated);

        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Warning));
        assert_eq!(status.seconds_remaining, Some(10));
    }

    #[test]
    fn apply_config_preserves_postponed_remaining_time_above_break_interval() {
        let mut config = BreakEngineConfig::load();
        config.allow_postpone = true;
        config.postpone_options = vec![
            super::PostponeOption {
                duration: 5,
                unit: "minutes".into(),
                seconds: 5 * 60,
            },
            super::PostponeOption {
                duration: 10,
                unit: "minutes".into(),
                seconds: 10 * 60,
            },
        ];
        config.break_interval = 1;

        let mut engine = BreakEngine::new(config.clone());
        engine.start();
        engine.begin_break_now();
        let postponed = engine
            .postpone_break_with_override(Some(10 * 60))
            .expect("10-minute postpone should be accepted");
        assert_eq!(postponed.seconds_remaining, Some(10 * 60));

        engine.apply_config(config);

        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(10 * 60));
    }

    fn reload_engine_from_path(
        engine: &mut BreakEngine,
        path: &Path,
        default_yaml: &str,
    ) -> Result<(), String> {
        let config = BreakEngineConfig::load_or_create_from_path(path, default_yaml)?;
        engine.apply_config(config);
        Ok(())
    }

    #[test]
    fn load_from_path_reports_yaml_errors_without_mutating_existing_engine() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        engine.tick(3);
        engine.begin_break_now();
        engine.set_idle(true);
        engine.set_fullscreen(true);
        let before = engine.config().clone();
        let before_snapshot = engine.snapshot(0);
        let temp = unique_test_dir("invalid-config");
        let config_path = temp.path().join("config.yaml");
        std::fs::write(&config_path, "not: [valid").unwrap();

        let error = reload_engine_from_path(&mut engine, &config_path, "short_break_interval: 1\n")
            .unwrap_err();

        assert!(error.contains("unknown field `not`"));
        assert_eq!(engine.config(), &before);
        assert_eq!(engine.snapshot(0), before_snapshot);
    }

    #[test]
    fn loads_persist_state_from_yaml() {
        let yaml = r#"
meta:
  config_version: "6.0.4"
random_order: true
allow_postpone: true
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
persist_state: true
postpone_duration: 5
postpone_unit: minutes
postpone_options:
  - duration: 5
    unit: minutes
  - duration: 10
    unit: minutes
  - duration: 15
    unit: minutes
strict_break: false
consecutive_skip_limit: 2
idle_time: 5
disable_options:
  - label: for_x_minutes
    time: 30
    unit: minute
  - label: for_x_hour
    time: 1
    unit: hour
  - label: for_x_hours
    time: 2
    unit: hour
  - label: for_x_hours
    time: 3
    unit: hour
short_breaks:
  - name: Gently close your eyes
long_breaks:
  - name: Walk for a while
"#;

        let config = BreakEngineConfig::from_yaml(yaml).unwrap();

        assert!(config.persist_state);
    }

    #[test]
    fn config_defaults_log_level_to_off() {
        let yaml = r#"
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
"#;

        let config = BreakEngineConfig::from_yaml(yaml).unwrap();

        assert_eq!(config.log_level, crate::logger::LogLevel::Off);
    }

    #[test]
    fn config_parses_log_level_when_present() {
        let yaml = r#"
log_level: debug
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
"#;

        let config = BreakEngineConfig::from_yaml(yaml).unwrap();

        assert_eq!(config.log_level, crate::logger::LogLevel::Debug);
    }

    #[test]
    fn config_invalid_log_level_falls_back_to_off() {
        let yaml = r#"
log_level: noisy
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
"#;

        let config = BreakEngineConfig::from_yaml(yaml).unwrap();

        assert_eq!(config.log_level, crate::logger::LogLevel::Off);
    }

    #[test]
    fn rejects_removed_shortcut_config_fields() {
        let yaml = r#"
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
shortcut_disable_time: 2
"#;

        let error = BreakEngineConfig::from_yaml(yaml).unwrap_err();

        assert!(error.to_string().contains("shortcut_disable_time"));
    }

    #[test]
    fn large_interval_ratio_clamps_short_break_count() {
        let yaml = r#"
short_break_interval: 1
long_break_interval: 300
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
consecutive_skip_limit: 2
"#;

        let config = BreakEngineConfig::from_yaml(yaml).unwrap();

        assert_eq!(config.no_of_short_breaks_per_long_break, u8::MAX);
    }

    #[test]
    fn snapshot_round_trip_preserves_runtime_state() {
        let mut config = BreakEngineConfig::load();
        config.random_order = false;
        config.short_breaks = vec![
            super::BreakTemplate { name: "A".into() },
            super::BreakTemplate { name: "B".into() },
            super::BreakTemplate { name: "C".into() },
        ];
        config.long_breaks = vec![super::BreakTemplate { name: "L".into() }];
        config.no_of_short_breaks_per_long_break = 99;

        let mut engine = BreakEngine::new(config.clone());
        engine.start();
        let first_break = engine.begin_break_now();
        assert_eq!(first_break.template_name.as_deref(), Some("A"));

        engine.skip_break().unwrap();
        let second_break = engine.begin_break_now();
        assert_eq!(second_break.template_name.as_deref(), Some("B"));
        engine.set_idle(true);
        engine.set_fullscreen(true);

        let snapshot = engine.snapshot(0);
        let mut restored = BreakEngine::from_snapshot(config, snapshot);
        let current_break = restored.current_break().expect("expected active break");

        assert_eq!(current_break.kind, BreakKind::Short);
        assert_eq!(
            current_break.duration_seconds,
            second_break.duration_seconds
        );
        assert_eq!(current_break.template_name, None);
        assert_eq!(
            restored.status().seconds_remaining,
            Some(second_break.duration_seconds)
        );
        assert_eq!(restored.status().phase, EnginePhase::OnBreak);

        restored.skip_break().unwrap();
        let next_break = restored.begin_break_now();
        assert_eq!(next_break.template_name.as_deref(), Some("C"));
        let error = restored.skip_break().unwrap_err();

        assert!(error.contains("Skip limit"));
    }

    #[test]
    fn restored_running_engine_keeps_advancing_after_import() {
        let mut config = BreakEngineConfig::load();
        config.random_order = false;
        config.short_breaks = vec![super::BreakTemplate { name: "A".into() }];
        config.no_of_short_breaks_per_long_break = 99;

        let mut engine = BreakEngine::new(config.clone());
        engine.start();
        engine.begin_break_now();

        let snapshot = engine.snapshot(123);
        let mut restored = BreakEngine::from_snapshot(config, snapshot);
        restored.rewind_last_sync_by(3);

        let status = restored.status();

        assert_eq!(status.phase, EnginePhase::OnBreak);
        assert_eq!(status.seconds_remaining, Some(12));
    }

    #[test]
    fn restore_running_snapshot_applies_elapsed_wall_clock_time() {
        let config = restore_test_config();
        let snapshot = restore_snapshot(EnginePhase::Running, 20, 0, 0, 0);
        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        let status = restored.restore_elapsed(7);

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(13));
    }

    #[test]
    fn restore_warning_snapshot_enters_break_when_elapsed_crosses_zero() {
        let config = restore_test_config();
        let snapshot = restore_snapshot(EnginePhase::Warning, 20, 3, 0, 0);
        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        let status = restored.restore_elapsed(3);

        assert!(matches!(status.phase, EnginePhase::OnBreak));
        assert_eq!(status.seconds_remaining, Some(8));
        assert_eq!(
            status.current_break.as_ref().map(|info| &info.kind),
            Some(&BreakKind::Short)
        );
    }

    #[test]
    fn restore_disabled_snapshot_expires_when_elapsed_is_long_enough() {
        let config = restore_test_config();
        let snapshot = restore_snapshot(EnginePhase::Disabled, 20, 0, 0, 4);
        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        let status = restored.restore_elapsed(4);

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(20));
    }

    #[test]
    fn restore_active_break_snapshot_completes_break_when_elapsed_is_long_enough() {
        let config = restore_test_config();
        let snapshot = restore_snapshot(EnginePhase::OnBreak, 20, 0, 3, 0);
        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        let status = restored.restore_elapsed(3);

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(120));
        assert!(status.current_break.is_none());
    }

    #[test]
    fn restored_running_snapshot_keeps_skip_limit_continuity() {
        let config = restore_test_config();
        let mut snapshot = restore_snapshot(EnginePhase::Running, 20, 0, 0, 0);
        snapshot.consecutive_skips = config.consecutive_skip_limit;
        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        let status = restored.status();
        assert!(matches!(status.phase, EnginePhase::Running));
        assert!(status.skip_limit_reached);

        restored.begin_break_now();
        let error = restored.skip_break().unwrap_err();

        assert!(error.contains("Skip limit"));
    }

    #[test]
    fn invalid_snapshot_template_orders_are_normalized_safely() {
        let mut config = BreakEngineConfig::load();
        config.random_order = false;
        config.short_breaks = vec![
            super::BreakTemplate { name: "A".into() },
            super::BreakTemplate { name: "B".into() },
        ];
        config.long_breaks = vec![super::BreakTemplate { name: "L".into() }];

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Running,
            work_remaining: config.break_interval * 60,
            warning_remaining: 0,
            break_remaining: 0,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 5,
            next_long_index: 0,
            short_break_order: vec![0, 2, 1],
            long_break_order: vec![0],
            current_break: None,
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 4,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);
        let break_info = restored.begin_break_now();

        assert_eq!(break_info.template_name.as_deref(), Some("A"));
    }

    #[test]
    fn malformed_started_snapshot_with_zero_second_break_is_normalized() {
        let mut config = BreakEngineConfig::load();
        config.random_order = false;
        config.short_breaks = vec![super::BreakTemplate { name: "A".into() }];
        let expected_work_remaining = config.break_interval * 60;

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::OnBreak,
            work_remaining: 100,
            warning_remaining: 0,
            break_remaining: 0,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 15,
                mandatory: false,
                template_name: Some("A".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 4,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Running));
        assert_eq!(
            restored.status().seconds_remaining,
            Some(expected_work_remaining)
        );
        assert!(restored.current_break().is_none());
        assert_eq!(restored.break_remaining, 0);
    }

    #[test]
    fn malformed_warning_snapshot_clears_stale_break_and_disabled_state() {
        let config = BreakEngineConfig::load();
        let expected_work_remaining = config.break_interval * 60;

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Warning,
            work_remaining: 100,
            warning_remaining: 0,
            break_remaining: 7,
            disabled_remaining: 11,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 15,
                mandatory: false,
                template_name: Some("A".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 4,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Running));
        assert_eq!(
            restored.status().seconds_remaining,
            Some(expected_work_remaining)
        );
        assert!(restored.current_break().is_none());
        assert_eq!(restored.break_remaining, 0);
        assert_eq!(restored.disabled_remaining, 0);
    }

    #[test]
    fn malformed_started_warning_snapshot_clears_stale_break_payload() {
        let config = BreakEngineConfig::load();

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Warning,
            work_remaining: 100,
            warning_remaining: 5,
            break_remaining: 7,
            disabled_remaining: 11,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 15,
                mandatory: false,
                template_name: Some("A".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 4,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Warning));
        assert_eq!(restored.status().seconds_remaining, Some(5));
        assert!(restored.current_break().is_none());
        assert_eq!(restored.break_remaining, 0);
        assert_eq!(restored.disabled_remaining, 0);
        assert_eq!(restored.consecutive_skips, 4);
    }

    #[test]
    fn malformed_started_disabled_snapshot_clears_stale_break_payload() {
        let config = BreakEngineConfig::load();

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Disabled,
            work_remaining: 100,
            warning_remaining: 0,
            break_remaining: 7,
            disabled_remaining: 9,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 15,
                mandatory: false,
                template_name: Some("A".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 2,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Disabled));
        assert_eq!(restored.status().seconds_remaining, Some(9));
        assert!(restored.current_break().is_none());
        assert_eq!(restored.break_remaining, 0);
        assert_eq!(restored.consecutive_skips, 2);
    }

    #[test]
    fn malformed_started_on_break_snapshot_sanitizes_active_break_payload() {
        let config = BreakEngineConfig::load();

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::OnBreak,
            work_remaining: 100,
            warning_remaining: 0,
            break_remaining: 7,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Long,
                duration_seconds: 999,
                mandatory: true,
                template_name: Some("Malformed".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 0,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);
        let current_break = restored.current_break().expect("expected active break");

        assert!(matches!(restored.status().phase, EnginePhase::OnBreak));
        assert_eq!(current_break.duration_seconds, 7);
        assert!(!current_break.mandatory);
    }

    #[test]
    fn malformed_started_running_snapshot_clears_break_only_state() {
        let config = BreakEngineConfig::load();

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Running,
            work_remaining: 100,
            warning_remaining: 4,
            break_remaining: 7,
            disabled_remaining: 9,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Long,
                duration_seconds: 999,
                mandatory: true,
                template_name: Some("Malformed".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 5,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Running));
        assert_eq!(restored.status().seconds_remaining, Some(100));
        assert!(restored.current_break().is_none());
        assert_eq!(restored.break_remaining, 0);
        assert_eq!(restored.warning_remaining, 0);
        assert_eq!(restored.disabled_remaining, 0);
        assert_eq!(restored.consecutive_skips, 5);
    }

    #[test]
    fn malformed_started_running_snapshot_with_zero_work_remaining_is_reset() {
        let config = BreakEngineConfig::load();
        let expected_work_remaining = config.break_interval * 60;

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Running,
            work_remaining: 0,
            warning_remaining: 0,
            break_remaining: 0,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: None,
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 0,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Running));
        assert_eq!(
            restored.status().seconds_remaining,
            Some(expected_work_remaining)
        );
        assert!(restored.current_break().is_none());
    }

    #[test]
    fn malformed_started_on_break_snapshot_normalizes_break_payload_fields() {
        let config = BreakEngineConfig::load();

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::OnBreak,
            work_remaining: 100,
            warning_remaining: 0,
            break_remaining: config.long_break_duration,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 999,
                mandatory: false,
                template_name: Some("Malformed".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 0,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config.clone(), snapshot);
        let current_break = restored.current_break().expect("expected active break");

        assert!(matches!(restored.status().phase, EnginePhase::OnBreak));
        assert_eq!(current_break.duration_seconds, config.long_break_duration);
        assert_eq!(current_break.mandatory, config.strict_break);
        assert_eq!(current_break.template_name, None);
    }

    #[test]
    fn equal_duration_active_break_is_classified_by_cycle_state() {
        let mut config = BreakEngineConfig::load();
        config.short_break_duration = 30;
        config.long_break_duration = 30;
        config.short_breaks = vec![super::BreakTemplate { name: "A".into() }];
        config.long_breaks = vec![super::BreakTemplate { name: "L".into() }];

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::OnBreak,
            work_remaining: 100,
            warning_remaining: 0,
            break_remaining: 30,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 30,
                mandatory: false,
                template_name: Some("Malformed".into()),
            }),
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 0,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);
        let current_break = restored.current_break().expect("expected active break");

        assert!(matches!(restored.status().phase, EnginePhase::OnBreak));
        assert_eq!(current_break.kind, BreakKind::Long);
        assert_eq!(current_break.duration_seconds, 30);
        assert_eq!(current_break.template_name, None);
    }

    #[test]
    fn stopped_snapshot_reinitializes_template_orders_for_new_session() {
        let mut config = BreakEngineConfig::load();
        config.random_order = false;

        let snapshot = super::BreakEngineSnapshot {
            was_started: false,
            phase: EnginePhase::OnBreak,
            work_remaining: 1,
            warning_remaining: 2,
            break_remaining: 3,
            disabled_remaining: 4,
            shorts_since_long: 5,
            next_short_index: 1,
            next_long_index: 1,
            short_break_order: vec![2, 1, 0],
            long_break_order: vec![1, 0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 15,
                mandatory: false,
                template_name: Some("A".into()),
            }),
            idle_active: true,
            idle_elapsed_seconds: 10,
            fullscreen: true,
            consecutive_skips: 2,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config.clone(), snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Stopped));
        assert_eq!(restored.current_break().is_none(), true);
        assert_eq!(restored.shorts_since_long, 0);
        assert_eq!(restored.next_short_index, 0);
        assert_eq!(restored.next_long_index, 0);
        assert_eq!(
            restored.short_break_order,
            (0..config.short_breaks.len()).collect::<Vec<_>>()
        );
        assert_eq!(
            restored.long_break_order,
            (0..config.long_breaks.len()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn malformed_shorts_since_long_is_clamped_on_import() {
        let mut config = BreakEngineConfig::load();
        config.random_order = false;
        config.short_breaks = vec![super::BreakTemplate { name: "A".into() }];
        config.long_breaks = vec![super::BreakTemplate { name: "L".into() }];
        config.no_of_short_breaks_per_long_break = 2;

        let snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Running,
            work_remaining: config.break_interval * 60,
            warning_remaining: 0,
            break_remaining: 0,
            disabled_remaining: 0,
            shorts_since_long: 9,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: None,
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 0,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert_eq!(restored.shorts_since_long, 2);
        assert_eq!(restored.status().upcoming_break_kind, Some(BreakKind::Long));
    }

    #[test]
    fn malformed_started_snapshot_is_normalized_on_import() {
        let config = BreakEngineConfig::load();
        let expected_work_remaining = config.break_interval * 60;

        let on_break_snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::OnBreak,
            work_remaining: 100,
            warning_remaining: 6,
            break_remaining: 7,
            disabled_remaining: 0,
            shorts_since_long: 2,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: None,
            idle_active: true,
            idle_elapsed_seconds: 42,
            fullscreen: true,
            consecutive_skips: 3,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config.clone(), on_break_snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Running));
        assert_eq!(
            restored.status().seconds_remaining,
            Some(expected_work_remaining)
        );
        assert!(restored.current_break().is_none());
        assert_eq!(restored.break_remaining, 0);
        assert_eq!(restored.disabled_remaining, 0);
        assert_eq!(restored.warning_remaining, 0);
        assert_eq!(restored.consecutive_skips, 3);

        let disabled_snapshot = super::BreakEngineSnapshot {
            was_started: true,
            phase: EnginePhase::Disabled,
            work_remaining: 100,
            warning_remaining: 0,
            break_remaining: 0,
            disabled_remaining: 0,
            shorts_since_long: 1,
            next_short_index: 0,
            next_long_index: 0,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: None,
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            consecutive_skips: 0,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, disabled_snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Running));
        assert_eq!(
            restored.status().seconds_remaining,
            Some(expected_work_remaining)
        );
        assert!(restored.current_break().is_none());
        assert_eq!(restored.disabled_remaining, 0);
    }

    #[test]
    fn stopped_snapshot_import_resets_active_runtime_state() {
        let config = BreakEngineConfig::load();

        let snapshot = super::BreakEngineSnapshot {
            was_started: false,
            phase: EnginePhase::OnBreak,
            work_remaining: 1,
            warning_remaining: 2,
            break_remaining: 3,
            disabled_remaining: 4,
            shorts_since_long: 5,
            next_short_index: 1,
            next_long_index: 1,
            short_break_order: vec![0],
            long_break_order: vec![0],
            current_break: Some(super::BreakInfo {
                kind: BreakKind::Short,
                duration_seconds: 15,
                mandatory: false,
                template_name: Some("A".into()),
            }),
            idle_active: true,
            idle_elapsed_seconds: 10,
            fullscreen: true,
            consecutive_skips: 2,
            saved_at_unix_seconds: 0,
        };

        let mut restored = BreakEngine::from_snapshot(config, snapshot);

        assert!(matches!(restored.status().phase, EnginePhase::Stopped));
        assert_eq!(restored.status().seconds_remaining, None);
        assert!(restored.current_break().is_none());
        assert_eq!(restored.work_remaining, restored.config.break_interval * 60);
        assert_eq!(restored.warning_remaining, 0);
        assert_eq!(restored.break_remaining, 0);
        assert_eq!(restored.disabled_remaining, 0);
        assert!(!restored.idle_active);
        assert_eq!(restored.idle_elapsed_seconds, 0);
        assert!(!restored.fullscreen);
        assert_eq!(restored.consecutive_skips, 0);
    }

    #[test]
    fn computes_break_distribution_and_rotation() {
        let mut config = BreakEngineConfig::load();
        config.random_order = false;
        let mut engine = BreakEngine::new(config);
        engine.start();

        let first = engine.debug_force_break();
        engine.complete_break().unwrap();
        let second = engine.debug_force_break();
        engine.complete_break().unwrap();
        let third = engine.debug_force_break();
        engine.complete_break().unwrap();
        let fourth = engine.debug_force_break();
        engine.complete_break().unwrap();
        let fifth = engine.debug_force_break();

        assert_eq!(
            first.template_name.as_deref(),
            Some("Gently close your eyes")
        );
        assert_eq!(
            second.template_name.as_deref(),
            Some("Roll your eyes a few times to each side")
        );
        assert_eq!(
            third.template_name.as_deref(),
            Some("Rotate your eyes in clockwise direction")
        );
        assert_eq!(
            fourth.template_name.as_deref(),
            Some("Rotate your eyes in counterclockwise direction")
        );
        assert_eq!(fifth.template_name.as_deref(), Some("Walk for a while"));
        assert!(matches!(fifth.kind, BreakKind::Long));
    }

    #[test]
    fn random_order_shuffles_short_break_templates_when_enabled() {
        let mut config = BreakEngineConfig::load();
        config.short_breaks = vec![
            super::BreakTemplate { name: "A".into() },
            super::BreakTemplate { name: "B".into() },
            super::BreakTemplate { name: "C".into() },
            super::BreakTemplate { name: "D".into() },
        ];
        config.long_breaks = vec![super::BreakTemplate { name: "L".into() }];
        config.no_of_short_breaks_per_long_break = 99;
        config.random_order = true;

        let mut engine = BreakEngine::new(config);
        engine.set_random_seed_for_tests(7);
        engine.start();

        let first = engine.debug_force_break();
        engine.complete_break().unwrap();
        let second = engine.debug_force_break();
        engine.complete_break().unwrap();
        let third = engine.debug_force_break();
        engine.complete_break().unwrap();
        let fourth = engine.debug_force_break();

        let seen = vec![
            first.template_name.unwrap(),
            second.template_name.unwrap(),
            third.template_name.unwrap(),
            fourth.template_name.unwrap(),
        ];

        assert_ne!(seen, vec!["A", "B", "C", "D"]);
        assert_eq!(seen.len(), 4);
        assert!(seen.contains(&"A".to_string()));
        assert!(seen.contains(&"B".to_string()));
        assert!(seen.contains(&"C".to_string()));
        assert!(seen.contains(&"D".to_string()));
    }

    #[test]
    fn enters_warning_before_break_and_then_starts_break() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        let work_seconds = engine.config.break_interval * 60;
        let warning_seconds = engine.config.pre_break_warning_time;

        let status = engine.advance_by(work_seconds - warning_seconds);
        assert!(matches!(status.phase, EnginePhase::Warning));
        assert_eq!(status.seconds_remaining, Some(warning_seconds));

        let status = engine.advance_by(warning_seconds);
        assert!(matches!(status.phase, EnginePhase::OnBreak));
        assert!(status.current_break.is_some());
    }

    #[test]
    fn strict_break_disables_skip() {
        let mut config = BreakEngineConfig::load();
        config.strict_break = true;

        let mut engine = BreakEngine::new(config);
        engine.start();
        engine.debug_force_break();

        let error = engine.skip_break().unwrap_err();
        assert!(error.contains("mandatory"));
    }

    #[test]
    fn consecutive_skip_limit_blocks_excessive_skips() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();

        engine.begin_break_now();
        engine.skip_break().unwrap();
        engine.begin_break_now();
        engine.skip_break().unwrap();

        let info = engine.begin_break_now();
        assert!(matches!(info.kind, BreakKind::Short | BreakKind::Long));
        let err = engine.skip_break().unwrap_err();
        assert!(err.contains("Skip limit"));
    }

    #[test]
    fn consecutive_skip_limit_resets_after_completion() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();

        engine.begin_break_now();
        engine.skip_break().unwrap();
        engine.begin_break_now();
        engine.complete_break().unwrap();

        assert_eq!(engine.consecutive_skips, 0);

        engine.begin_break_now();
        engine.skip_break().unwrap();
    }

    #[test]
    fn postpone_break_is_rejected_when_config_disables_it() {
        let mut config = BreakEngineConfig::load();
        config.allow_postpone = false;

        let mut engine = BreakEngine::new(config);
        engine.start();
        engine.begin_break_now();

        let error = engine.postpone_break().unwrap_err();

        assert!(error.contains("Postpone"));
    }

    #[test]
    fn postpone_break_reschedules_work_without_consuming_skip() {
        let mut config = BreakEngineConfig::load();
        config.allow_postpone = true;
        config.postpone_duration_seconds = 5 * 60;
        config.postpone_options = vec![super::PostponeOption {
            duration: 5,
            unit: "minutes".into(),
            seconds: 5 * 60,
        }];

        let mut engine = BreakEngine::new(config);
        engine.start();
        engine.begin_break_now();

        let status = engine.postpone_break_with_override(None).unwrap();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(5 * 60));
        assert!(status.current_break.is_none());
        assert!(!status.can_skip);
        assert!(!status.can_postpone);
    }

    #[test]
    fn postpone_break_rejects_unconfigured_override() {
        let mut config = BreakEngineConfig::load();
        config.allow_postpone = true;
        config.postpone_duration_seconds = 5 * 60;
        config.postpone_options = vec![super::PostponeOption {
            duration: 5,
            unit: "minutes".into(),
            seconds: 5 * 60,
        }];

        let mut engine = BreakEngine::new(config);
        engine.start();
        engine.begin_break_now();

        let error = engine
            .postpone_break_with_override(Some(10 * 60))
            .unwrap_err();

        assert!(error.contains("configured"));
    }

    #[test]
    fn idle_and_fullscreen_postpone_warning_and_break() {
        let mut config = BreakEngineConfig::load();
        config.idle_time = 0;
        let mut engine = BreakEngine::new(config);
        engine.start();
        engine.set_idle(true);
        let work_seconds = engine.config.break_interval * 60;
        let warning_seconds = engine.config.pre_break_warning_time;

        let status = engine.advance_by(work_seconds);
        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.postpone_reason.as_deref(), Some("idle"));
        assert_eq!(status.seconds_remaining, Some(warning_seconds));

        engine.set_idle(false);
        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Warning));

        engine.set_fullscreen(true);
        let status = engine.advance_by(warning_seconds);
        assert!(matches!(status.phase, EnginePhase::Warning));
        assert_eq!(status.postpone_reason.as_deref(), Some("fullscreen"));

        engine.set_fullscreen(false);
        let status = engine.advance_by(warning_seconds);
        assert!(matches!(status.phase, EnginePhase::OnBreak));
    }

    #[test]
    fn idle_only_postpones_after_idle_threshold() {
        let mut config = BreakEngineConfig::load();
        config.break_interval = 4;
        config.idle_time = 2;
        let mut engine = BreakEngine::new(config);
        engine.start();
        let work_seconds = engine.config.break_interval * 60;
        let warning_seconds = engine.config.pre_break_warning_time;
        let threshold_seconds = 2 * 60;
        engine.advance_by(work_seconds.saturating_sub(threshold_seconds + warning_seconds + 1));

        engine.set_idle(true);
        engine.advance_by(threshold_seconds - 1);
        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.postpone_reason, None);
        assert_eq!(status.seconds_remaining, Some(warning_seconds + 2));

        engine.advance_by(1);
        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.postpone_reason.as_deref(), Some("idle"));
        assert_eq!(status.seconds_remaining, Some(warning_seconds + 1));
    }

    #[test]
    fn disable_options_pause_engine_until_window_expires() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        engine.disable_for(30 * 60).unwrap();

        let status = engine.advance_by(10);
        assert!(matches!(status.phase, EnginePhase::Disabled));

        let status = engine.advance_by(30 * 60);
        assert!(matches!(status.phase, EnginePhase::Running));
    }

    #[test]
    fn begin_break_now_creates_active_break_for_manual_open() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();

        let info = engine.begin_break_now();

        assert!(matches!(info.kind, BreakKind::Short));
        assert_eq!(info.duration_seconds, 15);
        assert!(engine.current_break().is_some());
    }

    #[test]
    fn status_advances_with_elapsed_wall_clock_time() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        engine.rewind_last_sync_by(3);
        let work_seconds = engine.config.break_interval * 60;

        let status = engine.status();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(work_seconds - 3));
    }

    #[test]
    fn completing_after_break_elapsed_is_treated_as_finished() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        let info = engine.begin_break_now();
        engine.rewind_last_sync_by(info.duration_seconds);

        let result = engine.complete_break();

        assert!(result.is_ok());
        assert!(matches!(engine.status().phase, EnginePhase::Running));
    }

    #[test]
    fn reports_upcoming_break_kind_from_running_cycle() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();

        assert_eq!(engine.status().upcoming_break_kind, Some(BreakKind::Short));

        for _ in 0..4 {
            engine.debug_force_break();
            engine.complete_break().unwrap();
        }

        assert_eq!(engine.status().upcoming_break_kind, Some(BreakKind::Long));
    }
}
