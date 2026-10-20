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
    Ignored,
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
        if self.state == AnimatedSurfaceState::FallbackRequired {
            return Err("la superficie requiere una nueva activación explícita");
        }
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

    pub(crate) fn begin_explicit_create(&mut self) -> Result<(), &'static str> {
        if self.state == AnimatedSurfaceState::FallbackRequired {
            self.failures = 0;
            self.cleanup_started = false;
            self.state = AnimatedSurfaceState::Closed;
        }
        self.begin_create()
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
        if matches!(
            self.state,
            AnimatedSurfaceState::Closing
                | AnimatedSurfaceState::Closed
                | AnimatedSurfaceState::FallbackRequired
        ) {
            return FailureDecision::Ignored;
        }
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
        if self.state != AnimatedSurfaceState::FallbackRequired {
            self.state = AnimatedSurfaceState::Closed;
        }
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
pub(crate) enum DesktopLifecycleState {
    Active,
    Suspending,
    Suspended,
    Resuming,
    SessionLocked,
    SessionUnlocked,
    HostInvalid,
    Recovering,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesktopLifecycleEvent {
    Suspend,
    Resume,
    SessionLock,
    SessionUnlock,
    HostInvalid,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesktopLifecycleAction {
    SuspendSurfaces,
    RevalidateHost,
    NoAction,
    CloseSurfaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DesktopLifecycle {
    state: DesktopLifecycleState,
}

impl DesktopLifecycle {
    pub(crate) fn new() -> Self {
        Self {
            state: DesktopLifecycleState::Active,
        }
    }

    pub(crate) fn state(self) -> DesktopLifecycleState {
        self.state
    }

    pub(crate) fn event(&mut self, event: DesktopLifecycleEvent) -> DesktopLifecycleAction {
        use DesktopLifecycleAction::{CloseSurfaces, NoAction, RevalidateHost, SuspendSurfaces};
        use DesktopLifecycleEvent::{
            Close, HostInvalid, Resume, SessionLock, SessionUnlock, Suspend,
        };
        use DesktopLifecycleState::{
            Active, Closed, Closing, HostInvalid as Invalid, Recovering, Resuming, SessionLocked,
            SessionUnlocked, Suspended, Suspending,
        };

        match (self.state, event) {
            (Active, Suspend) | (Active, SessionLock) => {
                self.state = Suspending;
                self.state = if event == SessionLock {
                    SessionLocked
                } else {
                    Suspended
                };
                SuspendSurfaces
            }
            (Suspending, Suspend) | (Suspended, Suspend) | (SessionLocked, SessionLock) => NoAction,
            (Suspended, Resume) | (SessionLocked, SessionUnlock) => {
                self.state = if event == SessionUnlock {
                    SessionUnlocked
                } else {
                    Resuming
                };
                RevalidateHost
            }
            (SessionUnlocked, Resume) | (Resuming, Resume) => RevalidateHost,
            (Active, HostInvalid) | (Resuming, HostInvalid) => {
                self.state = Invalid;
                SuspendSurfaces
            }
            (Suspended, HostInvalid) | (SessionLocked, HostInvalid) | (Invalid, HostInvalid) => {
                self.state = Invalid;
                NoAction
            }
            (Invalid, Resume) => {
                self.state = Resuming;
                RevalidateHost
            }
            (Active, Close)
            | (Suspending, Close)
            | (Suspended, Close)
            | (SessionLocked, Close)
            | (SessionUnlocked, Close)
            | (Resuming, Close)
            | (Invalid, Close)
            | (Closing, Close) => {
                self.state = Closing;
                CloseSurfaces
            }
            (Closed, Close) | (Closed, _) | (Closing, _) => NoAction,
            (Recovering, Close) => {
                self.state = Closing;
                CloseSurfaces
            }
            (Recovering, _) => NoAction,
            (Active, Resume) | (Active, SessionUnlock) | (SessionUnlocked, SessionUnlock) => {
                NoAction
            }
            _ => NoAction,
        }
    }

    pub(crate) fn host_revalidated(&mut self, valid: bool) -> DesktopLifecycleAction {
        if !matches!(
            self.state,
            DesktopLifecycleState::Resuming | DesktopLifecycleState::SessionUnlocked
        ) {
            return DesktopLifecycleAction::NoAction;
        }
        if valid {
            self.state = DesktopLifecycleState::Recovering;
            DesktopLifecycleAction::RevalidateHost
        } else {
            self.state = DesktopLifecycleState::HostInvalid;
            DesktopLifecycleAction::NoAction
        }
    }

    pub(crate) fn recovery_finished(&mut self, success: bool) {
        if self.state == DesktopLifecycleState::Recovering {
            self.state = if success {
                DesktopLifecycleState::Active
            } else {
                DesktopLifecycleState::HostInvalid
            };
        }
    }

    pub(crate) fn finish_close(&mut self) {
        self.state = DesktopLifecycleState::Closed;
    }

    pub(crate) fn mark_closed(&mut self) {
        self.state = DesktopLifecycleState::Closed;
    }
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
        AnimatedSurfaceLifecycle, AnimatedSurfaceState, DesktopLifecycle, DesktopLifecycleAction,
        DesktopLifecycleEvent, DesktopLifecycleState, FailureDecision, RecoveryLifecycle,
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
        assert_eq!(lifecycle.state(), AnimatedSurfaceState::FallbackRequired);
        assert_eq!(
            lifecycle.fail(SurfaceFailureKind::WebView),
            FailureDecision::Ignored
        );
        assert!(lifecycle.begin_create().is_err());
        lifecycle.begin_explicit_create().unwrap();
        assert_eq!(lifecycle.state(), AnimatedSurfaceState::Creating);
    }

    #[test]
    fn late_media_error_after_close_is_ignored_and_never_reopens_surface() {
        let mut lifecycle = AnimatedSurfaceLifecycle::new();
        lifecycle.begin_create().unwrap();
        lifecycle.webview_ready().unwrap();
        lifecycle.playing().unwrap();
        lifecycle.begin_close();
        lifecycle.finish_close();
        assert_eq!(
            lifecycle.fail(SurfaceFailureKind::Media),
            FailureDecision::Ignored
        );
        assert_eq!(lifecycle.state(), AnimatedSurfaceState::Closed);
    }

    #[test]
    fn explicit_close_after_media_error_is_idempotent() {
        let mut lifecycle = AnimatedSurfaceLifecycle::new();
        lifecycle.begin_create().unwrap();
        lifecycle.webview_ready().unwrap();
        lifecycle.playing().unwrap();
        assert_eq!(
            lifecycle.fail(SurfaceFailureKind::Media),
            FailureDecision::RetryAllowed
        );
        lifecycle.begin_close();
        lifecycle.begin_close();
        lifecycle.finish_close();
        lifecycle.finish_close();
        assert_eq!(lifecycle.state(), AnimatedSurfaceState::Closed);
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

    #[test]
    fn desktop_lifecycle_suspends_and_revalidates_without_recreating_concurrently() {
        let mut lifecycle = DesktopLifecycle::new();
        assert_eq!(lifecycle.state(), DesktopLifecycleState::Active);
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::Suspend),
            DesktopLifecycleAction::SuspendSurfaces
        );
        assert_eq!(lifecycle.state(), DesktopLifecycleState::Suspended);
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::Suspend),
            DesktopLifecycleAction::NoAction
        );
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::Resume),
            DesktopLifecycleAction::RevalidateHost
        );
        assert_eq!(
            lifecycle.host_revalidated(true),
            DesktopLifecycleAction::RevalidateHost
        );
        lifecycle.recovery_finished(true);
        assert_eq!(lifecycle.state(), DesktopLifecycleState::Active);
    }

    #[test]
    fn desktop_lifecycle_lock_unlock_and_invalid_host_are_safe() {
        let mut lifecycle = DesktopLifecycle::new();
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::HostInvalid),
            DesktopLifecycleAction::SuspendSurfaces
        );
        assert_eq!(lifecycle.state(), DesktopLifecycleState::HostInvalid);
        lifecycle.event(DesktopLifecycleEvent::Resume);
        lifecycle.host_revalidated(true);
        lifecycle.recovery_finished(true);
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::SessionLock),
            DesktopLifecycleAction::SuspendSurfaces
        );
        assert_eq!(lifecycle.state(), DesktopLifecycleState::SessionLocked);
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::SessionLock),
            DesktopLifecycleAction::NoAction
        );
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::SessionUnlock),
            DesktopLifecycleAction::RevalidateHost
        );
        assert_eq!(lifecycle.state(), DesktopLifecycleState::SessionUnlocked);
        assert_eq!(
            lifecycle.host_revalidated(false),
            DesktopLifecycleAction::NoAction
        );
        assert_eq!(lifecycle.state(), DesktopLifecycleState::HostInvalid);
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::Resume),
            DesktopLifecycleAction::RevalidateHost
        );
        assert_eq!(
            lifecycle.host_revalidated(true),
            DesktopLifecycleAction::RevalidateHost
        );
        lifecycle.recovery_finished(true);
        assert_eq!(lifecycle.state(), DesktopLifecycleState::Active);
    }

    #[test]
    fn desktop_lifecycle_close_is_idempotent_after_suspend_or_close() {
        let mut lifecycle = DesktopLifecycle::new();
        lifecycle.event(DesktopLifecycleEvent::Suspend);
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::Close),
            DesktopLifecycleAction::CloseSurfaces
        );
        assert_eq!(lifecycle.state(), DesktopLifecycleState::Closing);
        lifecycle.finish_close();
        assert_eq!(
            lifecycle.event(DesktopLifecycleEvent::Close),
            DesktopLifecycleAction::NoAction
        );
        assert_eq!(lifecycle.state(), DesktopLifecycleState::Closed);
    }
}
