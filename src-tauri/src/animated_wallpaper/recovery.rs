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
    use super::{RecoveryLifecycle, RecoveryScenario, RecoveryState};

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
