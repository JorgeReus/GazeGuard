use serde::{Deserialize, Serialize};
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
pub struct BreakEngineConfig {
    pub break_interval: u64,
    pub long_break_duration: u64,
    pub no_of_short_breaks_per_long_break: u8,
    pub pre_break_warning_time: u64,
    pub short_break_duration: u64,
    pub strict_break: bool,
    pub idle_time: u64,
    pub short_breaks: Vec<BreakTemplate>,
    pub long_breaks: Vec<BreakTemplate>,
    pub disable_options: Vec<DisableOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RawBreakEngineConfig {
    short_break_interval: u64,
    long_break_interval: u64,
    long_break_duration: u64,
    pre_break_warning_time: u64,
    short_break_duration: u64,
    strict_break: bool,
    #[serde(default)]
    idle_time: u64,
    #[serde(default)]
    short_breaks: Vec<BreakTemplate>,
    #[serde(default)]
    long_breaks: Vec<BreakTemplate>,
    #[serde(default)]
    disable_options: Vec<DisableOption>,
}

impl BreakEngineConfig {
    pub fn load() -> Self {
        Self::from_yaml(include_str!("../gen/android/app/src/main/assets/config/defaults.yaml"))
            .expect("defaults config should be valid YAML")
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
                .saturating_sub(1) as u8
        };

        Self {
            break_interval: raw.short_break_interval,
            long_break_duration: raw.long_break_duration,
            no_of_short_breaks_per_long_break: breaks_per_long,
            pre_break_warning_time: raw.pre_break_warning_time,
            short_break_duration: raw.short_break_duration,
            strict_break: raw.strict_break,
            idle_time: raw.idle_time,
            short_breaks: raw.short_breaks,
            long_breaks: raw.long_breaks,
            disable_options: raw.disable_options,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnginePhase {
    Stopped,
    Running,
    Warning,
    OnBreak,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    pub disable_options: Vec<DisableOption>,
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
    current_break: Option<BreakInfo>,
    idle_active: bool,
    idle_elapsed_seconds: u64,
    fullscreen: bool,
    last_synced_at: Option<Instant>,
}

impl BreakEngine {
    pub fn new(config: BreakEngineConfig) -> Self {
        let interval = config.break_interval.saturating_mul(60);
        Self {
            config,
            phase: EnginePhase::Stopped,
            work_remaining: interval,
            warning_remaining: 0,
            break_remaining: 0,
            disabled_remaining: 0,
            shorts_since_long: 0,
            next_short_index: 0,
            next_long_index: 0,
            current_break: None,
            idle_active: false,
            idle_elapsed_seconds: 0,
            fullscreen: false,
            last_synced_at: None,
        }
    }

    pub fn start(&mut self) -> EngineStatus {
        self.phase = EnginePhase::Running;
        self.work_remaining = self.config.break_interval.saturating_mul(60);
        self.warning_remaining = 0;
        self.break_remaining = 0;
        self.disabled_remaining = 0;
        self.current_break = None;
        self.idle_elapsed_seconds = 0;
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
        self.last_synced_at = None;
        self.status()
    }

    #[cfg(test)]
    pub fn advance_by(&mut self, seconds: u64) -> EngineStatus {
        self.advance_by_seconds(seconds);
        self.status()
    }

    pub fn status(&mut self) -> EngineStatus {
        self.sync_with_clock();
        self.reconcile();
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
            can_skip: self.current_break.is_some() && !self.config.strict_break,
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
        self.finish_break_cycle();
        Ok(self.status())
    }

    pub fn complete_break(&mut self) -> Result<EngineStatus, String> {
        self.sync_with_clock();
        if !matches!(self.phase, EnginePhase::OnBreak) {
            return Ok(self.status());
        }
        self.finish_break_cycle();
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

    fn next_template_name(&mut self, kind: BreakKind) -> Option<String> {
        match kind {
            BreakKind::Short => {
                if self.config.short_breaks.is_empty() {
                    None
                } else {
                    let template = self.config.short_breaks[self.next_short_index].name.clone();
                    self.next_short_index = (self.next_short_index + 1) % self.config.short_breaks.len();
                    Some(template)
                }
            }
            BreakKind::Long => {
                if self.config.long_breaks.is_empty() {
                    None
                } else {
                    let template = self.config.long_breaks[self.next_long_index].name.clone();
                    self.next_long_index = (self.next_long_index + 1) % self.config.long_breaks.len();
                    Some(template)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BreakEngine, BreakEngineConfig, BreakKind, EnginePhase};

    #[test]
    fn loads_yaml_defaults_shape() {
        let config = BreakEngineConfig::load();

        assert_eq!(config.break_interval, 15);
        assert_eq!(config.pre_break_warning_time, 10);
        assert_eq!(config.short_break_duration, 15);
        assert_eq!(config.long_break_duration, 60);
        assert_eq!(config.no_of_short_breaks_per_long_break, 4);
        assert_eq!(config.idle_time, 5);
        assert!(!config.strict_break);
        assert_eq!(config.short_breaks.len(), 7);
        assert_eq!(config.long_breaks.len(), 2);
        assert_eq!(config.disable_options.len(), 4);
        assert_eq!(config.disable_options[0].seconds(), 30 * 60);
        assert_eq!(config.short_breaks[0].name, "Gently close your eyes");
        assert_eq!(config.long_breaks[0].name, "Walk for a while");
    }

    #[test]
    fn computes_break_distribution_and_rotation() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
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

        assert_eq!(first.template_name.as_deref(), Some("Gently close your eyes"));
        assert_eq!(second.template_name.as_deref(), Some("Roll your eyes a few times to each side"));
        assert_eq!(third.template_name.as_deref(), Some("Rotate your eyes in clockwise direction"));
        assert_eq!(fourth.template_name.as_deref(), Some("Rotate your eyes in counterclockwise direction"));
        assert_eq!(fifth.template_name.as_deref(), Some("Walk for a while"));
        assert!(matches!(fifth.kind, BreakKind::Long));
    }

    #[test]
    fn enters_warning_before_break_and_then_starts_break() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();

        let status = engine.advance_by(15 * 60 - 10);
        assert!(matches!(status.phase, EnginePhase::Warning));
        assert_eq!(status.seconds_remaining, Some(10));

        let status = engine.advance_by(10);
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
    fn idle_and_fullscreen_postpone_warning_and_break() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        engine.set_idle(true);

        let status = engine.advance_by(15 * 60);
        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.postpone_reason.as_deref(), Some("idle"));
        assert_eq!(status.seconds_remaining, Some(10));

        engine.set_idle(false);
        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Warning));

        engine.set_fullscreen(true);
        let status = engine.advance_by(10);
        assert!(matches!(status.phase, EnginePhase::Warning));
        assert_eq!(status.postpone_reason.as_deref(), Some("fullscreen"));

        engine.set_fullscreen(false);
        let status = engine.advance_by(10);
        assert!(matches!(status.phase, EnginePhase::OnBreak));
    }

    #[test]
    fn idle_only_postpones_after_idle_threshold() {
        let mut config = BreakEngineConfig::load();
        config.idle_time = 2;
        let mut engine = BreakEngine::new(config);
        engine.start();
        engine.advance_by((15 * 60) - 130);

        engine.set_idle(true);
        engine.advance_by((2 * 60) - 1);
        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.postpone_reason, None);
        assert_eq!(status.seconds_remaining, Some(11));

        engine.advance_by(1);
        let status = engine.status();
        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.postpone_reason.as_deref(), Some("idle"));
        assert_eq!(status.seconds_remaining, Some(10));
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

        let status = engine.status();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(15 * 60 - 3));
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
