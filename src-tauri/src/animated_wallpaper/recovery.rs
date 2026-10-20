const MAX_RECOVERY_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryProbe {
    Immediate,
    LowFrequency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecoveryPolicy {
    consecutive_failures: u8,
    exhausted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StartupRecoveryPlan {
    pub(crate) discard_native_references: bool,
    pub(crate) clean_owned_temporaries: bool,
    pub(crate) start_watcher_only_after_validation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimatedSurfaceState {
    Creating,
    WebViewReady,
    Playing,
    MediaError,
    WebViewError,
    Closing,
    Closed,
    FallbackRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceFailureKind {
    Media,
    WebView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureDecision {
    RetryAllowed,
    FallbackRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnimatedSurfaceLifecycle {
    state: AnimatedSurfaceState,
    failures: u8,
    cleanup_started: bool,
}

impl AnimatedSurfaceLifecycle {
    pub(crate) fn new() -> Self {
        Self {
            state: AnimatedSurfaceState::Closed,
            failures: 0,
            cleanup_started: false,
        }
    }

    pub(crate) fn begin_create(&mut self) -> Result<(), &'static str> {
        if self.cleanup_started
            || matches!(
                self.state,
                AnimatedSurfaceState::Creating
                    | AnimatedSurfaceState::WebViewReady
                    | AnimatedSurfaceState::Playing
            )
        {
            return Err("la superficie ya está activa o cerrándose");
        }
        self.state = AnimatedSurfaceState::Creating;
        Ok(())
    }

    pub(crate) fn webview_ready(&mut self) -> Result<(), &'static str> {
        if self.state != AnimatedSurfaceState::Creating {
            return Err("WebView2 no puede pasar a listo desde este estado");
        }
        self.state = AnimatedSurfaceState::WebViewReady;
        Ok(())
    }

    pub(crate) fn playing(&mut self) -> Result<(), &'static str> {
        if self.state != AnimatedSurfaceState::WebViewReady {
            return Err("la superficie no está lista para reproducir");
        }
        self.state = AnimatedSurfaceState::Playing;
        Ok(())
    }

    pub(crate) fn fail(&mut self, kind: SurfaceFailureKind) -> FailureDecision {
        self.state = match kind {
            SurfaceFailureKind::Media => AnimatedSurfaceState::MediaError,
            SurfaceFailureKind::WebView => AnimatedSurfaceState::WebViewError,
        };
        if self.failures < MAX_RECOVERY_ATTEMPTS {
            self.failures += 1;
        }
        if self.failures >= MAX_RECOVERY_ATTEMPTS {
            self.state = AnimatedSurfaceState::FallbackRequired;
            FailureDecision::FallbackRequired
        } else {
            FailureDecision::RetryAllowed
        }
    }

    pub(crate) fn begin_close(&mut self) {
        if !matches!(
            self.state,
            AnimatedSurfaceState::Closed | AnimatedSurfaceState::FallbackRequired
        ) {
            self.state = AnimatedSurfaceState::Closing;
        }
        self.cleanup_started = true;
    }

    pub(crate) fn finish_close(&mut self) {
        self.state = AnimatedSurfaceState::Closed;
        self.cleanup_started = false;
    }

    pub(crate) fn state(self) -> AnimatedSurfaceState {
        self.state
    }

    pub(crate) fn failures(self) -> u8 {
        self.failures
    }
}

impl StartupRecoveryPlan {
    pub(crate) fn for_new_process() -> Self {
        Self {
            discard_native_references: true,
            clean_owned_temporaries: true,
            start_watcher_only_after_validation: true,
        }
    }
}

impl RecoveryPolicy {
    pub(crate) fn new() -> Self {
        Self {
            consecutive_failures: 0,
            exhausted: false,
        }
    }

    pub(crate) fn probe(&self) -> RecoveryProbe {
        if self.exhausted {
            RecoveryProbe::LowFrequency
        } else {
            RecoveryProbe::Immediate
        }
    }

    pub(crate) fn on_failure(&mut self) {
        if self.consecutive_failures < MAX_RECOVERY_ATTEMPTS {
            self.consecutive_failures += 1;
        }
        if self.consecutive_failures >= MAX_RECOVERY_ATTEMPTS {
            self.exhausted = true;
        }
    }

    pub(crate) fn on_success(&mut self) {
        self.consecutive_failures = 0;
        self.exhausted = false;
    }

    pub(crate) fn attempts(self) -> u8 {
        self.consecutive_failures
    }

    pub(crate) fn exhausted(self) -> bool {
        self.exhausted
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryScenario {
    Success,
    HostAbsent,
    MultipleCandidates,
    SecondValidationFails,
}

impl RecoveryScenario {
    pub(crate) fn from_environment() -> Self {
        match std::env::var("LUMINA_ANIMATED_RECOVERY_SCENARIO")
            .unwrap_or_else(|_| "success".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "missing" | "absent" | "no-candidate" => Self::HostAbsent,
            "multiple" | "ambiguous" => Self::MultipleCandidates,
            "second-invalid" | "second-fails" => Self::SecondValidationFails,
            _ => Self::Success,
        }
    }

    pub(crate) fn simulated_candidate_count(self) -> Option<usize> {
        match self {
            Self::Success | Self::SecondValidationFails => None,
            Self::HostAbsent => Some(0),
            Self::MultipleCandidates => Some(2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryState {
    Unavailable,
    Active,
    Stopping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecoveryLifecycle {
    state: RecoveryState,
    renderer_count: usize,
}

impl RecoveryLifecycle {
    pub(crate) fn new() -> Self {
        Self {
            state: RecoveryState::Unavailable,
            renderer_count: 0,
        }
    }

    pub(crate) fn create_renderer(&mut self) -> Result<(), &'static str> {
        if self.renderer_count != 0 {
            return Err("ya existe un renderer activo");
        }
        self.renderer_count = 1;
        self.state = RecoveryState::Active;
        Ok(())
    }

    pub(crate) fn begin_cleanup(&mut self) {
        if self.renderer_count != 0 {
            self.state = RecoveryState::Stopping;
        }
    }

    pub(crate) fn finish_cleanup(&mut self) {
        self.renderer_count = 0;
        self.state = RecoveryState::Unavailable;
    }

    pub(crate) fn state(self) -> RecoveryState {
        self.state
    }

    pub(crate) fn renderer_count(self) -> usize {
        self.renderer_count
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AnimatedSurfaceLifecycle, AnimatedSurfaceState, FailureDecision, RecoveryLifecycle,
        RecoveryPolicy, RecoveryProbe, RecoveryScenario, RecoveryState, StartupRecoveryPlan,
        SurfaceFailureKind,
    };

    #[test]
    fn animated_surface_failure_enters_safe_state_and_cleanup_is_idempotent() {
        let mut lifecycle = AnimatedSurfaceLifecycle::new();
        lifecycle.begin_create().unwrap();
        lifecycle.webview_ready().unwrap();
        lifecycle.playing().unwrap();
        assert_eq!(
            lifecycle.fail(SurfaceFailureKind::Media),
            FailureDecision::RetryAllowed
        );
        lifecycle.begin_close();
        lifecycle.finish_close();
        lifecycle.begin_close();
        lifecycle.finish_close();
        assert_eq!(lifecycle.state(), AnimatedSurfaceState::Closed);
    }

    #[test]
    fn repeated_webview_failures_reach_fallback_without_infinite_retry() {
        let mut lifecycle = AnimatedSurfaceLifecycle::new();
        for _ in 0..2 {
            lifecycle.begin_create().unwrap();
            assert_eq!(
                lifecycle.fail(SurfaceFailureKind::WebView),
                FailureDecision::RetryAllowed
            );
            lifecycle.begin_close();
            lifecycle.finish_close();
        }
        lifecycle.begin_create().unwrap();
        assert_eq!(
            lifecycle.fail(SurfaceFailureKind::WebView),
            FailureDecision::FallbackRequired
        );
        assert_eq!(lifecycle.state(), AnimatedSurfaceState::FallbackRequired);
        assert_eq!(lifecycle.failures(), 3);
        lifecycle.begin_close();
        lifecycle.finish_close();
        assert!(lifecycle.begin_create().is_ok());
    }

    #[test]
    fn startup_plan_discards_stale_native_state_and_requires_new_host_validation() {
        let plan = StartupRecoveryPlan::for_new_process();
        assert!(plan.discard_native_references);
        assert!(plan.clean_owned_temporaries);
        assert!(plan.start_watcher_only_after_validation);
    }

    #[test]
    fn recovery_policy_enters_low_frequency_after_three_failures_and_restarts_after_success() {
        let mut policy = RecoveryPolicy::new();
        assert_eq!(policy.probe(), RecoveryProbe::Immediate);
        policy.on_failure();
        policy.on_failure();
        assert!(!policy.exhausted());
        policy.on_failure();
        assert_eq!(policy.attempts(), 3);
        assert!(policy.exhausted());
        assert_eq!(policy.probe(), RecoveryProbe::LowFrequency);
        policy.on_success();
        assert_eq!(policy.attempts(), 0);
        assert!(!policy.exhausted());
        assert_eq!(policy.probe(), RecoveryProbe::Immediate);
    }

    #[test]
    fn host_scenarios_map_to_safe_candidate_counts() {
        assert_eq!(
            RecoveryScenario::HostAbsent.simulated_candidate_count(),
            Some(0)
        );
        assert_eq!(
            RecoveryScenario::MultipleCandidates.simulated_candidate_count(),
            Some(2)
        );
        assert_eq!(RecoveryScenario::Success.simulated_candidate_count(), None);
    }

    #[test]
    fn a_second_renderer_is_blocked_until_cleanup_finishes() {
        let mut lifecycle = RecoveryLifecycle::new();
        lifecycle.create_renderer().unwrap();
        assert!(lifecycle.create_renderer().is_err());
        assert_eq!(lifecycle.renderer_count(), 1);
        lifecycle.begin_cleanup();
        assert_eq!(lifecycle.state(), RecoveryState::Stopping);
        lifecycle.finish_cleanup();
        assert_eq!(lifecycle.state(), RecoveryState::Unavailable);
        assert_eq!(lifecycle.renderer_count(), 0);
        lifecycle.create_renderer().unwrap();
        assert_eq!(lifecycle.renderer_count(), 1);
    }

    #[test]
    fn cleanup_is_idempotent() {
        let mut lifecycle = RecoveryLifecycle::new();
        lifecycle.create_renderer().unwrap();
        lifecycle.begin_cleanup();
        lifecycle.finish_cleanup();
        lifecycle.begin_cleanup();
        lifecycle.finish_cleanup();
        assert_eq!(lifecycle.state(), RecoveryState::Unavailable);
        assert_eq!(lifecycle.renderer_count(), 0);
    }
}
