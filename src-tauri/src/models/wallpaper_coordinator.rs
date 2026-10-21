use super::monitor_assignment::{
    AssignmentGeneration, MonitorAssignment, MonitorAssignmentRegistry, MonitorIdentity,
    MonitorResolution, WallpaperBackend,
};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssignmentOperation {
    pub(crate) assignment: MonitorAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingPhase {
    Prepared,
    Activated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingOperation {
    operation: AssignmentOperation,
    previous: Option<MonitorAssignment>,
    phase: PendingPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinatorResult {
    PreviousDeactivated,
    NoPreviousBackend,
    Activated,
    Committed,
    Released,
    Obsolete,
    AbortedPreviousActive,
    AbortedWithoutBackend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoordinatorError {
    InvalidMonitor,
    Unavailable,
    Ambiguous,
    AlreadyPending,
    BackendNotCreated,
    Obsolete,
    GenerationExhausted,
}

impl std::fmt::Display for CoordinatorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMonitor => "la identidad del monitor no es vÃ¡lida",
            Self::Unavailable => "el monitor no estÃ¡ disponible",
            Self::Ambiguous => "la identidad del monitor es ambigua",
            Self::AlreadyPending => "ya existe una operaciÃ³n pendiente para el monitor",
            Self::BackendNotCreated => "el backend fÃ­sico no fue creado",
            Self::Obsolete => "la operaciÃ³n de asignaciÃ³n ya es obsoleta",
            Self::GenerationExhausted => "se agotaron las generaciones de asignaciÃ³n",
        })
    }
}

impl std::error::Error for CoordinatorError {}

#[derive(Debug, Default)]
pub(crate) struct WallpaperCoordinator {
    registry: MonitorAssignmentRegistry,
    pending: BTreeMap<MonitorIdentity, PendingOperation>,
}

impl WallpaperCoordinator {
    pub(crate) fn reserve_resolved(
        &mut self,
        resolution: MonitorResolution,
        backend: WallpaperBackend,
    ) -> Result<AssignmentOperation, CoordinatorError> {
        match resolution {
            MonitorResolution::Resolved(identity) => self.reserve(identity, backend),
            MonitorResolution::Ambiguous => Err(CoordinatorError::Ambiguous),
            MonitorResolution::Unavailable => Err(CoordinatorError::Unavailable),
        }
    }

    pub(crate) fn reserve(
        &mut self,
        monitor: MonitorIdentity,
        backend: WallpaperBackend,
    ) -> Result<AssignmentOperation, CoordinatorError> {
        if monitor.as_str().trim().is_empty() {
            return Err(CoordinatorError::InvalidMonitor);
        }
        if self.pending.contains_key(&monitor) {
            return Err(CoordinatorError::AlreadyPending);
        }
        let assignment = self
            .registry
            .reserve(monitor.clone(), backend)
            .map_err(|_| CoordinatorError::GenerationExhausted)?;
        let operation = AssignmentOperation { assignment };
        self.pending.insert(
            monitor,
            PendingOperation {
                operation: operation.clone(),
                previous: self
                    .registry
                    .current(&operation.assignment.monitor)
                    .cloned(),
                phase: PendingPhase::Prepared,
            },
        );
        Ok(operation)
    }

    /// Confirms that the previous physical backend was deactivated. This is
    /// deliberately separate from `reserve`: preparing a generation must not
    /// destroy the currently effective assignment.
    pub(crate) fn confirm_previous_deactivated(
        &mut self,
        operation: &AssignmentOperation,
    ) -> Result<CoordinatorResult, CoordinatorError> {
        let Some(pending) = self.pending.get(operation.assignment.monitor_ref()) else {
            return Err(CoordinatorError::Obsolete);
        };
        if pending.operation != *operation {
            return Err(CoordinatorError::Obsolete);
        }
        if pending.previous.is_some() {
            self.registry.invalidate(operation.assignment.monitor_ref());
            Ok(CoordinatorResult::PreviousDeactivated)
        } else {
            Ok(CoordinatorResult::NoPreviousBackend)
        }
    }

    pub(crate) fn confirm_activated(
        &mut self,
        operation: &AssignmentOperation,
    ) -> Result<CoordinatorResult, CoordinatorError> {
        let Some(pending) = self.pending.get_mut(operation.assignment.monitor_ref()) else {
            return Err(CoordinatorError::Obsolete);
        };
        if pending.operation != *operation || pending.phase != PendingPhase::Prepared {
            return Err(CoordinatorError::Obsolete);
        }
        pending.phase = PendingPhase::Activated;
        Ok(CoordinatorResult::Activated)
    }

    pub(crate) fn commit(
        &mut self,
        operation: &AssignmentOperation,
    ) -> Result<CoordinatorResult, CoordinatorError> {
        let Some(pending) = self.pending.get(&operation.assignment.monitor) else {
            return Err(CoordinatorError::Obsolete);
        };
        if pending.operation != *operation || pending.phase != PendingPhase::Activated {
            return Err(CoordinatorError::Obsolete);
        }
        self.pending.remove(&operation.assignment.monitor);
        self.registry
            .activate(operation.assignment.clone())
            .map_err(|_| CoordinatorError::Obsolete)?;
        Ok(CoordinatorResult::Committed)
    }

    pub(crate) fn abort(&mut self, operation: &AssignmentOperation) -> CoordinatorResult {
        let Some(pending) = self.pending.get(operation.assignment.monitor_ref()) else {
            return CoordinatorResult::Obsolete;
        };
        if pending.operation != *operation {
            return CoordinatorResult::Obsolete;
        }
        let had_previous = pending.previous.is_some();
        let previous_is_active = self
            .registry
            .current(operation.assignment.monitor_ref())
            .is_some();
        self.pending.remove(operation.assignment.monitor_ref());
        match (had_previous, previous_is_active) {
            (true, true) => CoordinatorResult::AbortedPreviousActive,
            _ => CoordinatorResult::AbortedWithoutBackend,
        }
    }

    pub(crate) fn release(&mut self, operation: &AssignmentOperation) -> CoordinatorResult {
        if self.is_current(operation) {
            return self.invalidate(
                &operation.assignment.monitor,
                operation.assignment.generation,
            );
        }
        CoordinatorResult::Obsolete
    }

    pub(crate) fn current(&self, monitor: &MonitorIdentity) -> Option<&MonitorAssignment> {
        self.registry.current(monitor)
    }

    pub(crate) fn pending(&self, monitor: &MonitorIdentity) -> Option<&AssignmentOperation> {
        self.pending.get(monitor).map(|pending| &pending.operation)
    }

    pub(crate) fn is_current(&self, operation: &AssignmentOperation) -> bool {
        self.registry.is_current(&operation.assignment)
    }

    pub(crate) fn invalidate(
        &mut self,
        monitor: &MonitorIdentity,
        generation: AssignmentGeneration,
    ) -> CoordinatorResult {
        if self
            .pending
            .get(monitor)
            .is_some_and(|pending| pending.operation.assignment.generation == generation)
        {
            self.pending.remove(monitor);
            return CoordinatorResult::Released;
        }
        if self
            .registry
            .current(monitor)
            .is_some_and(|assignment| assignment.generation == generation)
        {
            self.registry.invalidate(monitor);
            return CoordinatorResult::Released;
        }
        CoordinatorResult::Obsolete
    }

    pub(crate) fn assignments(&self) -> Vec<&MonitorAssignment> {
        self.registry.assignments()
    }
}

pub(crate) fn global() -> &'static Mutex<WallpaperCoordinator> {
    static INSTANCE: OnceLock<Mutex<WallpaperCoordinator>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(WallpaperCoordinator::default()))
}

#[cfg(test)]
mod tests {
    use super::{CoordinatorError, CoordinatorResult, WallpaperCoordinator};
    use crate::models::monitor_assignment::{MonitorIdentity, MonitorResolution, WallpaperBackend};

    fn monitor(value: &str) -> MonitorIdentity {
        MonitorIdentity::new(value).unwrap()
    }

    fn activate(coordinator: &mut WallpaperCoordinator, operation: &super::AssignmentOperation) {
        coordinator.confirm_previous_deactivated(operation).unwrap();
        coordinator.confirm_activated(operation).unwrap();
        coordinator.commit(operation).unwrap();
    }

    #[test]
    fn different_monitors_can_use_different_backends() {
        let mut coordinator = WallpaperCoordinator::default();
        let a = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        let b = coordinator
            .reserve(monitor("native-b"), WallpaperBackend::Animated)
            .unwrap();
        activate(&mut coordinator, &a);
        activate(&mut coordinator, &b);
        assert_eq!(
            coordinator.current(&monitor("native-a")).unwrap().backend,
            WallpaperBackend::Static
        );
        assert_eq!(
            coordinator.current(&monitor("native-b")).unwrap().backend,
            WallpaperBackend::Animated
        );
    }

    #[test]
    fn replacing_backend_invalidates_the_previous_generation() {
        let mut coordinator = WallpaperCoordinator::default();
        let old = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &old);
        let new = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        assert!(coordinator.is_current(&old));
        assert!(!coordinator.is_current(&new));
        assert_eq!(coordinator.commit(&old), Err(CoordinatorError::Obsolete));
        activate(&mut coordinator, &new);
        assert_eq!(
            coordinator.current(&monitor("native-a")).unwrap().backend,
            WallpaperBackend::Animated
        );
    }

    #[test]
    fn stale_animated_result_cannot_overwrite_static_assignment() {
        let mut coordinator = WallpaperCoordinator::default();
        let animated = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        activate(&mut coordinator, &animated);
        let static_operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &static_operation);
        assert_eq!(
            coordinator.commit(&animated),
            Err(CoordinatorError::Obsolete)
        );
        assert_eq!(
            coordinator.current(&monitor("native-a")).unwrap().backend,
            WallpaperBackend::Static
        );
    }

    #[test]
    fn failed_physical_creation_does_not_become_effective() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        assert!(coordinator.current(&monitor("native-a")).is_none());
        assert_eq!(
            coordinator.abort(&operation),
            CoordinatorResult::AbortedWithoutBackend
        );
        assert!(coordinator.current(&monitor("native-a")).is_none());
    }

    #[test]
    fn pending_duplicate_is_rejected_and_abort_is_idempotent() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        assert_eq!(
            coordinator.reserve(monitor("native-a"), WallpaperBackend::Animated),
            Err(CoordinatorError::AlreadyPending)
        );
        assert_eq!(
            coordinator.abort(&operation),
            CoordinatorResult::AbortedWithoutBackend
        );
        assert_eq!(coordinator.abort(&operation), CoordinatorResult::Obsolete);
    }

    #[test]
    fn failed_replacement_before_deactivation_preserves_previous_backend() {
        let mut coordinator = WallpaperCoordinator::default();
        let static_operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &static_operation);
        let animated_operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        assert_eq!(
            coordinator.abort(&animated_operation),
            CoordinatorResult::AbortedPreviousActive
        );
        assert_eq!(
            coordinator.current(&monitor("native-a")).unwrap().backend,
            WallpaperBackend::Static
        );
    }

    #[test]
    fn failed_replacement_after_deactivation_reports_no_backend() {
        let mut coordinator = WallpaperCoordinator::default();
        let static_operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &static_operation);
        let animated_operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        assert_eq!(
            coordinator.confirm_previous_deactivated(&animated_operation),
            Ok(CoordinatorResult::PreviousDeactivated)
        );
        assert_eq!(
            coordinator.abort(&animated_operation),
            CoordinatorResult::AbortedWithoutBackend
        );
        assert!(coordinator.current(&monitor("native-a")).is_none());
    }

    #[test]
    fn activation_must_be_confirmed_before_commit() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        coordinator
            .confirm_previous_deactivated(&operation)
            .unwrap();
        assert_eq!(
            coordinator.commit(&operation),
            Err(CoordinatorError::Obsolete)
        );
        assert!(coordinator.current(&monitor("native-a")).is_none());
    }

    #[test]
    fn same_backend_replacement_gets_a_new_generation() {
        let mut coordinator = WallpaperCoordinator::default();
        let first = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &first);
        let second = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        assert!(second.assignment.generation.value() > first.assignment.generation.value());
        activate(&mut coordinator, &second);
        assert!(!coordinator.is_current(&first));
        assert!(coordinator.is_current(&second));
    }

    #[test]
    fn animated_to_static_replacement_is_exclusive() {
        let mut coordinator = WallpaperCoordinator::default();
        let animated = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        activate(&mut coordinator, &animated);
        let static_operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &static_operation);
        assert_eq!(
            coordinator.current(&monitor("native-a")).unwrap().backend,
            WallpaperBackend::Static
        );
        assert!(!coordinator.is_current(&animated));
    }

    #[test]
    fn second_replacement_is_rejected_while_first_is_pending() {
        let mut coordinator = WallpaperCoordinator::default();
        let first = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        assert_eq!(
            coordinator.reserve(monitor("native-a"), WallpaperBackend::Static),
            Err(CoordinatorError::AlreadyPending)
        );
        coordinator.abort(&first);
    }

    #[test]
    fn stale_activation_cannot_commit_after_a_newer_generation() {
        let mut coordinator = WallpaperCoordinator::default();
        let first = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        coordinator.abort(&first);
        let second = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        assert_eq!(
            coordinator.confirm_activated(&first),
            Err(CoordinatorError::Obsolete)
        );
        activate(&mut coordinator, &second);
    }

    #[test]
    fn deactivation_of_one_monitor_does_not_affect_another() {
        let mut coordinator = WallpaperCoordinator::default();
        let a = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        let b = coordinator
            .reserve(monitor("native-b"), WallpaperBackend::Animated)
            .unwrap();
        activate(&mut coordinator, &a);
        activate(&mut coordinator, &b);
        let replacement = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        coordinator
            .confirm_previous_deactivated(&replacement)
            .unwrap();
        assert!(coordinator.is_current(&b));
        assert!(coordinator.current(&monitor("native-a")).is_none());
    }

    #[test]
    fn abort_after_previous_deactivation_does_not_fake_a_rollback() {
        let mut coordinator = WallpaperCoordinator::default();
        let old = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        activate(&mut coordinator, &old);
        let replacement = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        coordinator
            .confirm_previous_deactivated(&replacement)
            .unwrap();
        assert_eq!(
            coordinator.abort(&replacement),
            CoordinatorResult::AbortedWithoutBackend
        );
        assert!(!coordinator.is_current(&old));
    }

    #[test]
    fn fallback_does_not_start_static_automatically() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        activate(&mut coordinator, &operation);
        coordinator.invalidate(&monitor("native-a"), operation.assignment.generation);
        assert!(coordinator.current(&monitor("native-a")).is_none());
    }

    #[test]
    fn ambiguous_and_unavailable_monitors_are_not_assignable() {
        let mut coordinator = WallpaperCoordinator::default();
        assert_eq!(
            coordinator.reserve_resolved(MonitorResolution::Ambiguous, WallpaperBackend::Static),
            Err(CoordinatorError::Ambiguous)
        );
        assert_eq!(
            coordinator
                .reserve_resolved(MonitorResolution::Unavailable, WallpaperBackend::Animated),
            Err(CoordinatorError::Unavailable)
        );
    }

    #[test]
    fn stale_release_cannot_remove_a_newer_assignment() {
        let mut coordinator = WallpaperCoordinator::default();
        let old = coordinator
            .reserve("native-a".into(), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &old);
        let new = coordinator
            .reserve("native-a".into(), WallpaperBackend::Animated)
            .unwrap();
        activate(&mut coordinator, &new);
        assert_eq!(coordinator.release(&old), CoordinatorResult::Obsolete);
        assert_eq!(
            coordinator.current(&"native-a".into()).unwrap().backend,
            WallpaperBackend::Animated
        );
    }
}
