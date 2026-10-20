use std::collections::BTreeMap;

use serde::Serialize;

pub(crate) const MAX_ACTIVE_ANIMATED_SURFACES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct LogicalBounds {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl LogicalBounds {
    pub(crate) fn right(self) -> i32 {
        self.x + self.width as i32
    }

    pub(crate) fn bottom(self) -> i32 {
        self.y + self.height as i32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct MonitorSpec {
    pub(crate) id: String,
    pub(crate) bounds: LogicalBounds,
    pub(crate) dpi: u32,
    pub(crate) primary: bool,
    pub(crate) simulated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceState {
    Active,
    Stopping,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostObservation {
    Stable,
    Changed,
    TransientError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostReaction {
    NoAction,
    Invalidated,
    RetryLater,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MonitorSurface {
    pub(crate) monitor_id: String,
    pub(crate) host_id: String,
    pub(crate) host_generation: u64,
    pub(crate) renderer_id: String,
    pub(crate) temp_id: String,
    pub(crate) state: SurfaceState,
    pub(crate) bounds: LogicalBounds,
    pub(crate) dpi: u32,
}

#[derive(Debug)]
pub(crate) struct MonitorCoordinator {
    surfaces: BTreeMap<String, MonitorSurface>,
    host_generation: u64,
    host_valid: bool,
}

impl Default for MonitorCoordinator {
    fn default() -> Self {
        Self {
            surfaces: BTreeMap::new(),
            host_generation: 1,
            host_valid: true,
        }
    }
}

impl MonitorCoordinator {
    pub(crate) fn create(&mut self, monitor: &MonitorSpec) -> Result<(), &'static str> {
        if self
            .surfaces
            .get(&monitor.id)
            .is_some_and(|surface| surface.state == SurfaceState::Active)
        {
            return Err("ya existe un renderer activo para este monitor");
        }
        if !self.host_valid {
            return Err("el host compartido no es válido");
        }
        if self.active_count() >= MAX_ACTIVE_ANIMATED_SURFACES {
            return Err("se alcanzÃ³ el lÃ­mite global de superficies animadas");
        }
        let sequence = self.surfaces.len() + 1;
        self.surfaces.insert(
            monitor.id.clone(),
            MonitorSurface {
                monitor_id: monitor.id.clone(),
                host_id: "host:shared-desktop".into(),
                host_generation: self.host_generation,
                renderer_id: format!("renderer:{}:{sequence}", monitor.id),
                temp_id: format!("temp:{}:{sequence}", monitor.id),
                state: SurfaceState::Active,
                bounds: monitor.bounds,
                dpi: monitor.dpi,
            },
        );
        Ok(())
    }

    pub(crate) fn begin_cleanup(&mut self, monitor_id: &str) {
        if let Some(surface) = self.surfaces.get_mut(monitor_id) {
            if surface.state == SurfaceState::Active {
                surface.state = SurfaceState::Stopping;
            }
        }
    }

    pub(crate) fn finish_cleanup(&mut self, monitor_id: &str) {
        if let Some(surface) = self.surfaces.get_mut(monitor_id) {
            surface.state = SurfaceState::Unavailable;
        }
    }

    pub(crate) fn mark_unavailable(&mut self, monitor_id: &str) {
        self.begin_cleanup(monitor_id);
        self.finish_cleanup(monitor_id);
    }

    pub(crate) fn invalidate_host(&mut self) {
        self.host_valid = false;
    }

    pub(crate) fn observe_host(&mut self, observation: HostObservation) -> HostReaction {
        match observation {
            HostObservation::Stable => HostReaction::NoAction,
            HostObservation::TransientError => HostReaction::RetryLater,
            HostObservation::Changed if self.host_valid => {
                self.invalidate_host();
                HostReaction::Invalidated
            }
            HostObservation::Changed => HostReaction::NoAction,
        }
    }

    pub(crate) fn rediscover_host(&mut self) {
        self.host_generation = self.host_generation.saturating_add(1);
        self.host_valid = true;
    }

    pub(crate) fn host_is_valid(&self) -> bool {
        self.host_valid
    }

    pub(crate) fn host_generation(&self) -> u64 {
        self.host_generation
    }

    pub(crate) fn update_geometry(
        &mut self,
        monitor_id: &str,
        bounds: LogicalBounds,
        dpi: u32,
    ) -> Result<(), &'static str> {
        let surface = self
            .surfaces
            .get_mut(monitor_id)
            .ok_or("monitor sin renderer")?;
        surface.bounds = bounds;
        surface.dpi = dpi;
        Ok(())
    }

    pub(crate) fn active_count(&self) -> usize {
        self.surfaces
            .values()
            .filter(|surface| surface.state == SurfaceState::Active)
            .count()
    }

    pub(crate) fn surface(&self, monitor_id: &str) -> Option<&MonitorSurface> {
        self.surfaces.get(monitor_id)
    }
}

pub(crate) fn simulated_topologies() -> [Vec<MonitorSpec>; 2] {
    [
        vec![
            MonitorSpec {
                id: "simulated-display-a".into(),
                bounds: LogicalBounds {
                    x: 0,
                    y: 0,
                    width: 2560,
                    height: 1440,
                },
                dpi: 144,
                primary: true,
                simulated: true,
            },
            MonitorSpec {
                id: "simulated-display-b".into(),
                bounds: LogicalBounds {
                    x: 2560,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                dpi: 96,
                primary: false,
                simulated: true,
            },
        ],
        vec![
            MonitorSpec {
                id: "simulated-display-left".into(),
                bounds: LogicalBounds {
                    x: -1920,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                dpi: 120,
                primary: false,
                simulated: true,
            },
            MonitorSpec {
                id: "simulated-display-main".into(),
                bounds: LogicalBounds {
                    x: 0,
                    y: 0,
                    width: 2560,
                    height: 1440,
                },
                dpi: 144,
                primary: true,
                simulated: true,
            },
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        simulated_topologies, HostObservation, HostReaction, LogicalBounds, MonitorCoordinator,
        MonitorSpec, SurfaceState,
    };

    fn monitor(id: &str, x: i32, dpi: u32) -> MonitorSpec {
        MonitorSpec {
            id: id.into(),
            bounds: LogicalBounds {
                x,
                y: 0,
                width: 1920,
                height: 1080,
            },
            dpi,
            primary: x == 0,
            simulated: true,
        }
    }

    #[test]
    fn calculates_bounds_without_using_a_global_rectangle() {
        let topology = &simulated_topologies()[0];
        assert_eq!(topology[0].bounds.right(), 2560);
        assert_eq!(topology[1].bounds.x, 2560);
        assert_eq!(topology[1].bounds.right(), 4480);
    }

    #[test]
    fn supports_negative_coordinates_and_multiple_dpi_values() {
        let topology = &simulated_topologies()[1];
        assert_eq!(topology[0].bounds.x, -1920);
        assert_eq!(topology[0].bounds.right(), 0);
        assert_eq!(topology[0].dpi, 120);
        assert_eq!(topology[1].dpi, 144);
    }

    #[test]
    fn keeps_two_monitor_lifecycles_independent() {
        let a = monitor("display-a", 0, 144);
        let b = monitor("display-b", 1920, 96);
        let mut coordinator = MonitorCoordinator::default();
        coordinator.create(&a).unwrap();
        coordinator.create(&b).unwrap();
        assert_eq!(coordinator.active_count(), 2);
        assert_eq!(
            coordinator.surface(&a.id).unwrap().host_id,
            coordinator.surface(&b.id).unwrap().host_id
        );
        assert_ne!(
            coordinator.surface(&a.id).unwrap().renderer_id,
            coordinator.surface(&b.id).unwrap().renderer_id
        );
        coordinator.mark_unavailable(&a.id);
        assert_eq!(coordinator.active_count(), 1);
        assert_eq!(
            coordinator.surface(&b.id).unwrap().state,
            SurfaceState::Active
        );
    }

    #[test]
    fn blocks_duplicate_renderer_per_monitor_but_allows_recreation_after_cleanup() {
        let a = monitor("display-a", 0, 96);
        let mut coordinator = MonitorCoordinator::default();
        coordinator.create(&a).unwrap();
        assert!(coordinator.create(&a).is_err());
        coordinator.mark_unavailable(&a.id);
        coordinator.create(&a).unwrap();
        assert_eq!(coordinator.active_count(), 1);
    }

    #[test]
    fn global_surface_limit_rejects_third_and_releases_after_cleanup() {
        let a = monitor("display-a", 0, 96);
        let b = monitor("display-b", 1920, 96);
        let c = monitor("display-c", 3840, 96);
        let mut coordinator = MonitorCoordinator::default();
        coordinator.create(&a).unwrap();
        coordinator.create(&b).unwrap();
        assert!(coordinator.create(&c).is_err());
        coordinator.mark_unavailable(&a.id);
        coordinator.create(&c).unwrap();
        assert_eq!(coordinator.active_count(), 2);
    }

    #[test]
    fn closing_a_does_not_invalidate_shared_host_or_b() {
        let a = monitor("display-a", 0, 144);
        let b = monitor("display-b", 1920, 96);
        let mut coordinator = MonitorCoordinator::default();
        coordinator.create(&a).unwrap();
        coordinator.create(&b).unwrap();
        let generation = coordinator.host_generation();
        coordinator.mark_unavailable(&a.id);
        assert!(coordinator.host_is_valid());
        assert_eq!(coordinator.host_generation(), generation);
        assert_eq!(
            coordinator.surface(&b.id).unwrap().state,
            SurfaceState::Active
        );
    }

    #[test]
    fn invalid_host_rejects_new_surface_until_rediscovery() {
        let a = monitor("display-a", 0, 144);
        let b = monitor("display-b", 1920, 96);
        let mut coordinator = MonitorCoordinator::default();
        coordinator.create(&a).unwrap();
        let old_generation = coordinator.host_generation();
        coordinator.invalidate_host();
        assert!(coordinator.create(&b).is_err());
        coordinator.rediscover_host();
        coordinator.create(&b).unwrap();
        assert!(coordinator.host_generation() > old_generation);
        assert_ne!(
            coordinator.surface(&a.id).unwrap().host_generation,
            coordinator.surface(&b.id).unwrap().host_generation
        );
    }

    #[test]
    fn repeated_cleanup_does_not_affect_the_other_surface() {
        let a = monitor("display-a", 0, 144);
        let b = monitor("display-b", 1920, 96);
        let mut coordinator = MonitorCoordinator::default();
        coordinator.create(&a).unwrap();
        coordinator.create(&b).unwrap();
        coordinator.mark_unavailable(&a.id);
        coordinator.mark_unavailable(&a.id);
        assert_eq!(coordinator.active_count(), 1);
        assert_eq!(
            coordinator.surface(&b.id).unwrap().state,
            SurfaceState::Active
        );
    }

    #[test]
    fn stable_host_does_not_trigger_repeated_recovery() {
        let mut coordinator = MonitorCoordinator::default();
        assert_eq!(
            coordinator.observe_host(HostObservation::Stable),
            HostReaction::NoAction
        );
        assert!(coordinator.host_is_valid());
        assert_eq!(
            coordinator.observe_host(HostObservation::TransientError),
            HostReaction::RetryLater
        );
        assert!(coordinator.host_is_valid());
    }

    #[test]
    fn changed_host_invalidates_only_once_until_rediscovery() {
        let mut coordinator = MonitorCoordinator::default();
        assert_eq!(
            coordinator.observe_host(HostObservation::Changed),
            HostReaction::Invalidated
        );
        assert_eq!(
            coordinator.observe_host(HostObservation::Changed),
            HostReaction::NoAction
        );
        let generation = coordinator.host_generation();
        coordinator.rediscover_host();
        assert_eq!(coordinator.host_generation(), generation + 1);
        assert_eq!(
            coordinator.observe_host(HostObservation::Stable),
            HostReaction::NoAction
        );
    }

    #[test]
    fn updates_only_the_selected_monitor_geometry() {
        let a = monitor("display-a", 0, 144);
        let b = monitor("display-b", 1920, 96);
        let mut coordinator = MonitorCoordinator::default();
        coordinator.create(&a).unwrap();
        coordinator.create(&b).unwrap();
        coordinator
            .update_geometry(
                &a.id,
                LogicalBounds {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                96,
            )
            .unwrap();
        assert_eq!(coordinator.surface(&a.id).unwrap().dpi, 96);
        assert_eq!(coordinator.surface(&b.id).unwrap().dpi, 96);
        assert_eq!(coordinator.surface(&b.id).unwrap().bounds.x, 1920);
    }
}
