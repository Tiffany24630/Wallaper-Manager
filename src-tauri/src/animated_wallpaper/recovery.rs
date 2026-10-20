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
        RecoveryLifecycle, RecoveryPolicy, RecoveryProbe, RecoveryScenario, RecoveryState,
        StartupRecoveryPlan,
    };

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
