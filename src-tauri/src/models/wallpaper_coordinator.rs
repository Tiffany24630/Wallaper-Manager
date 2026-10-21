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
    previous_deactivated: bool,
    phase: PendingPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinatorResult {
    PreviousDeactivated,
    NoPreviousBackend,
    Activated,
    Committed,
    CommittedResourceReadyVisibilityUnverified,
    Released,
    Obsolete,
    AbortedPreviousActive,
    AbortedWithoutBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssignmentEvidence {
    Confirmed,
    ResourceReadyVisibilityUnverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoordinatorError {
    NotInitialized,
    InitializationInProgress,
    AlreadyInitialized,
    ShuttingDown,
    Shutdown,
    InitializationFailed,
    InvalidMonitor,
    Unavailable,
    Ambiguous,
    AlreadyPending,
    BackendNotCreated,
    EvidenceIncompatible,
    PreviousBackendStillActive,
    Obsolete,
    GenerationExhausted,
}

impl std::fmt::Display for CoordinatorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotInitialized => "coordinator is not initialized",
            Self::InitializationInProgress => "coordinator initialization is in progress",
            Self::AlreadyInitialized => "coordinator is already initialized",
            Self::ShuttingDown => "coordinator is shutting down",
            Self::Shutdown => "coordinator is shut down",
            Self::InitializationFailed => "coordinator initialization failed",
            Self::InvalidMonitor => "la identidad del monitor no es vÃ¡lida",
            Self::Unavailable => "el monitor no estÃ¡ disponible",
            Self::Ambiguous => "la identidad del monitor es ambigua",
            Self::AlreadyPending => "ya existe una operaciÃ³n pendiente para el monitor",
            Self::BackendNotCreated => "el backend fÃ­sico no fue creado",
            Self::EvidenceIncompatible => "la evidencia no es compatible con el backend solicitado",
            Self::PreviousBackendStillActive => {
                "el backend animado anterior sigue activo fÃ­sicamente"
            }
            Self::Obsolete => "la operaciÃ³n de asignaciÃ³n ya es obsoleta",
            Self::GenerationExhausted => "se agotaron las generaciones de asignaciÃ³n",
        })
    }
}

impl std::error::Error for CoordinatorError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinatorLifecycle {
    Uninitialized,
    Initializing,
    Ready,
    ShuttingDown,
    Shutdown,
    InitializationFailed,
}

#[derive(Debug)]
pub(crate) struct WallpaperCoordinator {
    registry: MonitorAssignmentRegistry,
    pending: BTreeMap<MonitorIdentity, PendingOperation>,
    evidence: BTreeMap<MonitorIdentity, AssignmentEvidence>,
    lifecycle: CoordinatorLifecycle,
}

impl Default for WallpaperCoordinator {
    fn default() -> Self {
        // Local coordinators used by isolated model tests start ready. The
        // process-wide coordinator is explicitly initialized during startup.
        Self {
            registry: MonitorAssignmentRegistry::default(),
            pending: BTreeMap::new(),
            evidence: BTreeMap::new(),
            lifecycle: CoordinatorLifecycle::Ready,
        }
    }
}

impl WallpaperCoordinator {
    pub(crate) fn uninitialized() -> Self {
        Self {
            registry: MonitorAssignmentRegistry::default(),
            pending: BTreeMap::new(),
            evidence: BTreeMap::new(),
            lifecycle: CoordinatorLifecycle::Uninitialized,
        }
    }

    pub(crate) fn lifecycle(&self) -> CoordinatorLifecycle {
        self.lifecycle
    }

    pub(crate) fn initialize(&mut self) -> Result<(), CoordinatorError> {
        match self.lifecycle {
            CoordinatorLifecycle::Uninitialized => {
                self.lifecycle = CoordinatorLifecycle::Initializing;
                self.lifecycle = CoordinatorLifecycle::Ready;
                Ok(())
            }
            CoordinatorLifecycle::Initializing => Err(CoordinatorError::InitializationInProgress),
            CoordinatorLifecycle::Ready => Err(CoordinatorError::AlreadyInitialized),
            CoordinatorLifecycle::ShuttingDown | CoordinatorLifecycle::Shutdown => {
                Err(CoordinatorError::Shutdown)
            }
            CoordinatorLifecycle::InitializationFailed => {
                Err(CoordinatorError::InitializationFailed)
            }
        }
    }

    pub(crate) fn begin_shutdown(&mut self) -> Result<usize, CoordinatorError> {
        match self.lifecycle {
            CoordinatorLifecycle::Ready => {
                self.lifecycle = CoordinatorLifecycle::ShuttingDown;
                let cancelled = self.pending.len();
                self.pending.clear();
                Ok(cancelled)
            }
            CoordinatorLifecycle::ShuttingDown | CoordinatorLifecycle::Shutdown => Ok(0),
            CoordinatorLifecycle::Uninitialized | CoordinatorLifecycle::InitializationFailed => {
                self.lifecycle = CoordinatorLifecycle::Shutdown;
                Ok(0)
            }
            CoordinatorLifecycle::Initializing => {
                self.lifecycle = CoordinatorLifecycle::ShuttingDown;
                self.pending.clear();
                Ok(0)
            }
        }
    }

    pub(crate) fn finish_shutdown(&mut self) -> Result<(), CoordinatorError> {
        match self.lifecycle {
            CoordinatorLifecycle::ShuttingDown => {
                self.pending.clear();
                self.registry = MonitorAssignmentRegistry::default();
                self.evidence.clear();
                self.lifecycle = CoordinatorLifecycle::Shutdown;
                Ok(())
            }
            CoordinatorLifecycle::Shutdown => Ok(()),
            _ => Err(CoordinatorError::ShuttingDown),
        }
    }

    fn ensure_ready(&self) -> Result<(), CoordinatorError> {
        match self.lifecycle {
            CoordinatorLifecycle::Ready => Ok(()),
            CoordinatorLifecycle::Uninitialized => Err(CoordinatorError::NotInitialized),
            CoordinatorLifecycle::Initializing => Err(CoordinatorError::InitializationInProgress),
            CoordinatorLifecycle::ShuttingDown => Err(CoordinatorError::ShuttingDown),
            CoordinatorLifecycle::Shutdown => Err(CoordinatorError::Shutdown),
            CoordinatorLifecycle::InitializationFailed => {
                Err(CoordinatorError::InitializationFailed)
            }
        }
    }

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
        self.ensure_ready()?;
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
                previous_deactivated: false,
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
        self.ensure_ready()?;
        self.validate_previous_deactivated(operation)?;
        let previous = self
            .pending
            .get(operation.assignment.monitor_ref())
            .filter(|pending| {
                pending.operation == *operation
                    && pending.phase == PendingPhase::Prepared
                    && !pending.previous_deactivated
            })
            .and_then(|pending| pending.previous.clone());
        if previous.as_ref().is_some_and(|previous| {
            previous.backend == WallpaperBackend::Animated
                && self
                    .registry
                    .current(operation.assignment.monitor_ref())
                    .is_some_and(|current| current == previous)
        }) {
            return Err(CoordinatorError::PreviousBackendStillActive);
        }
        let had_previous = {
            let Some(pending) = self.pending.get_mut(operation.assignment.monitor_ref()) else {
                return Err(CoordinatorError::Obsolete);
            };
            if pending.operation != *operation
                || pending.phase != PendingPhase::Prepared
                || pending.previous_deactivated
            {
                return Err(CoordinatorError::Obsolete);
            }
            let had_previous = pending.previous.is_some();
            pending.previous_deactivated = true;
            had_previous
        };
        if had_previous {
            self.registry.invalidate(operation.assignment.monitor_ref());
            self.evidence.remove(operation.assignment.monitor_ref());
            Ok(CoordinatorResult::PreviousDeactivated)
        } else {
            Ok(CoordinatorResult::NoPreviousBackend)
        }
    }

    pub(crate) fn validate_previous_deactivated(
        &self,
        operation: &AssignmentOperation,
    ) -> Result<(), CoordinatorError> {
        self.ensure_ready()?;
        let Some(pending) = self.pending.get(operation.assignment.monitor_ref()) else {
            return Err(CoordinatorError::Obsolete);
        };
        if pending.operation != *operation
            || pending.phase != PendingPhase::Prepared
            || pending.previous_deactivated
        {
            return Err(CoordinatorError::Obsolete);
        }
        if pending.previous.as_ref().is_some_and(|previous| {
            previous.backend == WallpaperBackend::Animated
                && self
                    .registry
                    .current(operation.assignment.monitor_ref())
                    .is_some_and(|current| current == previous)
        }) {
            return Err(CoordinatorError::PreviousBackendStillActive);
        }
        Ok(())
    }

    pub(crate) fn confirm_activated(
        &mut self,
        operation: &AssignmentOperation,
    ) -> Result<CoordinatorResult, CoordinatorError> {
        self.ensure_ready()?;
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
        self.commit_with_evidence(operation, AssignmentEvidence::Confirmed)
    }

    pub(crate) fn commit_with_evidence(
        &mut self,
        operation: &AssignmentOperation,
        evidence: AssignmentEvidence,
    ) -> Result<CoordinatorResult, CoordinatorError> {
        self.ensure_ready()?;
        let Some(pending) = self.pending.get(&operation.assignment.monitor) else {
            return Err(CoordinatorError::Obsolete);
        };
        if pending.operation != *operation || pending.phase != PendingPhase::Activated {
            return Err(CoordinatorError::Obsolete);
        }
        let evidence_allowed = match operation.assignment.backend {
            WallpaperBackend::Static | WallpaperBackend::Unassigned => {
                evidence == AssignmentEvidence::Confirmed
            }
            WallpaperBackend::Animated => {
                evidence == AssignmentEvidence::ResourceReadyVisibilityUnverified
            }
        };
        if !evidence_allowed {
            return Err(CoordinatorError::EvidenceIncompatible);
        }
        self.pending.remove(&operation.assignment.monitor);
        self.registry
            .activate(operation.assignment.clone())
            .map_err(|_| CoordinatorError::Obsolete)?;
        self.evidence
            .insert(operation.assignment.monitor.clone(), evidence);
        Ok(match evidence {
            AssignmentEvidence::Confirmed => CoordinatorResult::Committed,
            AssignmentEvidence::ResourceReadyVisibilityUnverified => {
                CoordinatorResult::CommittedResourceReadyVisibilityUnverified
            }
        })
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

    pub(crate) fn evidence(&self, monitor: &MonitorIdentity) -> Option<AssignmentEvidence> {
        self.evidence.get(monitor).copied()
    }

    pub(crate) fn is_physically_confirmed(&self, monitor: &MonitorIdentity) -> bool {
        self.evidence(monitor) == Some(AssignmentEvidence::Confirmed)
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
            self.evidence.remove(monitor);
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
    INSTANCE.get_or_init(|| Mutex::new(WallpaperCoordinator::uninitialized()))
}

#[cfg(test)]
mod tests {
    use super::{
        AssignmentEvidence, CoordinatorError, CoordinatorLifecycle, CoordinatorResult,
        WallpaperCoordinator,
    };
    use crate::models::monitor_assignment::{MonitorIdentity, MonitorResolution, WallpaperBackend};

    fn monitor(value: &str) -> MonitorIdentity {
        MonitorIdentity::new(value).unwrap()
    }

    #[test]
    fn lifecycle_requires_explicit_initialization_for_process_coordinator() {
        let mut coordinator = WallpaperCoordinator::uninitialized();
        assert_eq!(coordinator.lifecycle(), CoordinatorLifecycle::Uninitialized);
        assert_eq!(
            coordinator.reserve(monitor("native-a"), WallpaperBackend::Static),
            Err(CoordinatorError::NotInitialized)
        );
        coordinator.initialize().unwrap();
        assert_eq!(coordinator.lifecycle(), CoordinatorLifecycle::Ready);
        assert_eq!(
            coordinator.initialize(),
            Err(CoordinatorError::AlreadyInitialized)
        );
    }

    #[test]
    fn shutdown_rejects_new_operations_and_cancels_pending_work() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        assert_eq!(coordinator.begin_shutdown(), Ok(1));
        assert_eq!(coordinator.lifecycle(), CoordinatorLifecycle::ShuttingDown);
        assert_eq!(
            coordinator.reserve(monitor("native-b"), WallpaperBackend::Animated),
            Err(CoordinatorError::ShuttingDown)
        );
        assert_eq!(coordinator.abort(&operation), CoordinatorResult::Obsolete);
        coordinator.finish_shutdown().unwrap();
        assert_eq!(coordinator.lifecycle(), CoordinatorLifecycle::Shutdown);
        assert_eq!(
            coordinator.reserve(monitor("native-a"), WallpaperBackend::Static),
            Err(CoordinatorError::Shutdown)
        );
    }

    #[test]
    fn stale_commit_cannot_succeed_after_shutdown() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        coordinator.begin_shutdown().unwrap();
        assert_eq!(
            coordinator.commit(&operation),
            Err(CoordinatorError::ShuttingDown)
        );
        coordinator.finish_shutdown().unwrap();
        assert_eq!(
            coordinator.commit(&operation),
            Err(CoordinatorError::Shutdown)
        );
    }

    #[test]
    fn shutdown_is_idempotent_and_does_not_reopen_coordinator() {
        let mut coordinator = WallpaperCoordinator::default();
        assert_eq!(coordinator.begin_shutdown(), Ok(0));
        assert_eq!(coordinator.begin_shutdown(), Ok(0));
        coordinator.finish_shutdown().unwrap();
        coordinator.finish_shutdown().unwrap();
        assert_eq!(coordinator.lifecycle(), CoordinatorLifecycle::Shutdown);
    }

    #[test]
    fn late_release_after_shutdown_cannot_change_coordinator_state() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Static)
            .unwrap();
        activate(&mut coordinator, &operation);
        coordinator.begin_shutdown().unwrap();
        coordinator.finish_shutdown().unwrap();
        assert_eq!(coordinator.release(&operation), CoordinatorResult::Obsolete);
        assert!(coordinator.current(&monitor("native-a")).is_none());
        assert_eq!(coordinator.lifecycle(), CoordinatorLifecycle::Shutdown);
    }

    #[test]
    fn resource_ready_does_not_claim_physical_visibility() {
        let mut unverified = WallpaperCoordinator::default();
        let animated = unverified
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        unverified.confirm_previous_deactivated(&animated).unwrap();
        unverified.confirm_activated(&animated).unwrap();
        assert_eq!(
            unverified
                .commit_with_evidence(
                    &animated,
                    AssignmentEvidence::ResourceReadyVisibilityUnverified,
                )
                .unwrap(),
            CoordinatorResult::CommittedResourceReadyVisibilityUnverified
        );
        assert_eq!(
            unverified.evidence(&monitor("native-a")),
            Some(AssignmentEvidence::ResourceReadyVisibilityUnverified)
        );
        assert!(!unverified.is_physically_confirmed(&monitor("native-a")));
    }

    #[test]
    fn animated_cannot_commit_as_physically_confirmed() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        coordinator
            .confirm_previous_deactivated(&operation)
            .unwrap();
        coordinator.confirm_activated(&operation).unwrap();
        assert_eq!(
            coordinator.commit(&operation),
            Err(CoordinatorError::EvidenceIncompatible)
        );
        assert!(coordinator.pending(&monitor("native-a")).is_some());
        assert!(coordinator.current(&monitor("native-a")).is_none());
    }

    #[test]
    fn previous_deactivation_cannot_be_confirmed_out_of_order() {
        let mut coordinator = WallpaperCoordinator::default();
        let operation = coordinator
            .reserve(monitor("native-a"), WallpaperBackend::Animated)
            .unwrap();
        coordinator
            .confirm_previous_deactivated(&operation)
            .unwrap();
        assert_eq!(
            coordinator.confirm_previous_deactivated(&operation),
            Err(CoordinatorError::Obsolete)
        );
        coordinator.confirm_activated(&operation).unwrap();
        assert_eq!(
            coordinator.confirm_previous_deactivated(&operation),
            Err(CoordinatorError::Obsolete)
        );
    }

    #[test]
    fn animated_failure_after_previous_deactivation_reports_no_backend() {
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

    fn activate(coordinator: &mut WallpaperCoordinator, operation: &super::AssignmentOperation) {
        coordinator.confirm_previous_deactivated(operation).unwrap();
        coordinator.confirm_activated(operation).unwrap();
        let evidence = match operation.assignment.backend {
            super::WallpaperBackend::Animated => {
                AssignmentEvidence::ResourceReadyVisibilityUnverified
            }
            super::WallpaperBackend::Static | super::WallpaperBackend::Unassigned => {
                AssignmentEvidence::Confirmed
            }
        };
        coordinator
            .commit_with_evidence(operation, evidence)
            .unwrap();
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
        coordinator.release(&animated);
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
        assert_eq!(
            coordinator.confirm_previous_deactivated(&static_operation),
            Err(CoordinatorError::PreviousBackendStillActive)
        );
        assert_eq!(coordinator.release(&animated), CoordinatorResult::Released);
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
        coordinator.release(&old);
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
