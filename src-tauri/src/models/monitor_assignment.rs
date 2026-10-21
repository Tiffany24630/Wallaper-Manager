use std::collections::BTreeMap;
use std::fmt;

/// Stable identity of a monitor within an assignment operation.
///
/// The value should be a native monitor identifier (for example the device
/// path returned by `IDesktopWallpaper`). Enumeration indexes are deliberately
/// not part of this type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub(crate) struct MonitorIdentity(String);

impl MonitorIdentity {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("la identidad del monitor no puede estar vacía");
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MonitorIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for MonitorIdentity {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for MonitorIdentity {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WallpaperBackend {
    Static,
    Animated,
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AssignmentGeneration(u64);

impl AssignmentGeneration {
    pub(crate) fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MonitorAssignment {
    pub(crate) monitor: MonitorIdentity,
    pub(crate) backend: WallpaperBackend,
    pub(crate) generation: AssignmentGeneration,
}

#[derive(Debug, Default)]
pub(crate) struct MonitorAssignmentRegistry {
    current: BTreeMap<MonitorIdentity, MonitorAssignment>,
    next_generation: BTreeMap<MonitorIdentity, u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MonitorGeometry {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl MonitorGeometry {
    pub(crate) fn new(x: i32, y: i32, width: u32, height: u32) -> Result<Self, &'static str> {
        if width == 0 || height == 0 {
            return Err("la geometría del monitor debe tener dimensiones positivas");
        }
        x.checked_add(width as i32)
            .ok_or("la geometría del monitor desborda el borde derecho")?;
        y.checked_add(height as i32)
            .ok_or("la geometría del monitor desborda el borde inferior")?;
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiMonitorDescriptor {
    pub(crate) name: Option<String>,
    pub(crate) geometry: MonitorGeometry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeMonitorDescriptor {
    pub(crate) device_path: String,
    pub(crate) source_name: Option<String>,
    pub(crate) geometry: Option<MonitorGeometry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MonitorResolution {
    Resolved(MonitorIdentity),
    Ambiguous,
    Unavailable,
}

pub(crate) fn resolve_monitor_identity(
    ui: &UiMonitorDescriptor,
    native: &[NativeMonitorDescriptor],
) -> Result<MonitorResolution, &'static str> {
    validate_native_descriptors(native)?;

    let named: Vec<&NativeMonitorDescriptor> = ui
        .name
        .as_deref()
        .map(|name| {
            native
                .iter()
                .filter(|candidate| candidate.source_name.as_deref() == Some(name))
                .collect()
        })
        .unwrap_or_default();
    if named.len() > 1 {
        return Ok(MonitorResolution::Ambiguous);
    }
    if let Some(candidate) = named.first() {
        return Ok(MonitorResolution::Resolved(MonitorIdentity::new(
            candidate.device_path.clone(),
        )?));
    }

    let geometric: Vec<&NativeMonitorDescriptor> = native
        .iter()
        .filter(|candidate| candidate.geometry == Some(ui.geometry))
        .collect();
    match geometric.as_slice() {
        [] => Ok(MonitorResolution::Unavailable),
        [candidate] => Ok(MonitorResolution::Resolved(MonitorIdentity::new(
            candidate.device_path.clone(),
        )?)),
        _ => Ok(MonitorResolution::Ambiguous),
    }
}

pub(crate) fn resolve_monitor_identities(
    ui: &[UiMonitorDescriptor],
    native: &[NativeMonitorDescriptor],
) -> Result<Vec<MonitorResolution>, &'static str> {
    validate_native_descriptors(native)?;
    let resolutions: Vec<MonitorResolution> = ui
        .iter()
        .map(|descriptor| resolve_monitor_identity(descriptor, native))
        .collect::<Result<_, _>>()?;
    let resolved: Vec<&MonitorIdentity> = resolutions
        .iter()
        .filter_map(|result| match result {
            MonitorResolution::Resolved(identity) => Some(identity),
            MonitorResolution::Ambiguous | MonitorResolution::Unavailable => None,
        })
        .collect();
    for (index, identity) in resolved.iter().enumerate() {
        if resolved[index + 1..].iter().any(|other| other == identity) {
            return Err("varios monitores de la UI corresponden al mismo monitor nativo");
        }
    }
    Ok(resolutions)
}

fn validate_native_descriptors(native: &[NativeMonitorDescriptor]) -> Result<(), &'static str> {
    for descriptor in native {
        MonitorIdentity::new(descriptor.device_path.clone())?;
        if let Some(geometry) = descriptor.geometry {
            MonitorGeometry::new(geometry.x, geometry.y, geometry.width, geometry.height)?;
        }
    }
    for (index, descriptor) in native.iter().enumerate() {
        if native[index + 1..]
            .iter()
            .any(|other| other.device_path == descriptor.device_path)
        {
            return Err("hay identificadores nativos de monitor duplicados");
        }
    }
    Ok(())
}

impl MonitorAssignmentRegistry {
    pub(crate) fn reserve(
        &mut self,
        monitor: MonitorIdentity,
        backend: WallpaperBackend,
    ) -> Result<MonitorAssignment, &'static str> {
        let next = self.next_generation.entry(monitor.clone()).or_insert(0);
        *next = next
            .checked_add(1)
            .ok_or("se agotaron las generaciones de asignaciÃ³n del monitor")?;
        Ok(MonitorAssignment {
            monitor,
            backend,
            generation: AssignmentGeneration(*next),
        })
    }

    pub(crate) fn activate(&mut self, assignment: MonitorAssignment) -> Result<(), &'static str> {
        let next = self
            .next_generation
            .get(&assignment.monitor)
            .copied()
            .unwrap_or_default();
        if next != assignment.generation.value() {
            return Err("la asignaciÃ³n ya no es la generaciÃ³n vigente");
        }
        self.current.insert(assignment.monitor.clone(), assignment);
        Ok(())
    }

    pub(crate) fn begin(
        &mut self,
        monitor: MonitorIdentity,
        backend: WallpaperBackend,
    ) -> Result<MonitorAssignment, &'static str> {
        let next = self.next_generation.entry(monitor.clone()).or_insert(0);
        *next = next
            .checked_add(1)
            .ok_or("se agotaron las generaciones de asignación del monitor")?;

        let assignment = MonitorAssignment {
            monitor: monitor.clone(),
            backend,
            generation: AssignmentGeneration(*next),
        };
        self.current.insert(monitor, assignment.clone());
        Ok(assignment)
    }

    pub(crate) fn current(&self, monitor: &MonitorIdentity) -> Option<&MonitorAssignment> {
        self.current.get(monitor)
    }

    pub(crate) fn is_current(&self, assignment: &MonitorAssignment) -> bool {
        self.current(assignment.monitor_ref()) == Some(assignment)
    }

    pub(crate) fn invalidate(&mut self, monitor: &MonitorIdentity) -> Option<MonitorAssignment> {
        self.current.remove(monitor)
    }

    pub(crate) fn compare(&self, left: &MonitorAssignment, right: &MonitorAssignment) -> bool {
        left == right
    }

    pub(crate) fn assignments(&self) -> Vec<&MonitorAssignment> {
        self.current.values().collect()
    }
}

impl MonitorAssignment {
    pub(crate) fn monitor_ref(&self) -> &MonitorIdentity {
        &self.monitor
    }
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_monitor_identity, AssignmentGeneration, MonitorAssignment,
        MonitorAssignmentRegistry, MonitorGeometry, MonitorIdentity, MonitorResolution,
        NativeMonitorDescriptor, UiMonitorDescriptor, WallpaperBackend,
    };

    fn monitor(value: &str) -> MonitorIdentity {
        MonitorIdentity::new(value).unwrap()
    }

    #[test]
    fn newer_operation_invalidates_older_operation_on_same_monitor() {
        let mut registry = MonitorAssignmentRegistry::default();
        let first = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Static)
            .unwrap();
        let second = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Animated)
            .unwrap();

        assert_eq!(first.generation.value(), 1);
        assert_eq!(second.generation.value(), 2);
        assert!(!registry.is_current(&first));
        assert!(registry.is_current(&second));
    }

    #[test]
    fn monitors_have_independent_generations() {
        let mut registry = MonitorAssignmentRegistry::default();
        let first_a = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Static)
            .unwrap();
        let first_b = registry
            .begin(monitor("DISPLAY2"), WallpaperBackend::Animated)
            .unwrap();
        let second_a = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Unassigned)
            .unwrap();

        assert_eq!(first_a.generation.value(), 1);
        assert_eq!(first_b.generation.value(), 1);
        assert_eq!(second_a.generation.value(), 2);
        assert!(!registry.is_current(&first_a));
        assert!(registry.is_current(&first_b));
        assert!(registry.is_current(&second_a));
    }

    #[test]
    fn invalidation_rejects_the_previous_generation() {
        let mut registry = MonitorAssignmentRegistry::default();
        let assignment = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Static)
            .unwrap();

        assert_eq!(
            registry.invalidate(&assignment.monitor),
            Some(assignment.clone())
        );
        assert!(!registry.is_current(&assignment));
        assert!(registry.current(&assignment.monitor).is_none());
    }

    #[test]
    fn backend_variants_are_distinct_without_running_a_backend() {
        let mut registry = MonitorAssignmentRegistry::default();
        let static_assignment = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Static)
            .unwrap();
        let animated_assignment = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Animated)
            .unwrap();
        let unassigned = registry
            .begin(monitor("DISPLAY1"), WallpaperBackend::Unassigned)
            .unwrap();

        assert_eq!(static_assignment.backend, WallpaperBackend::Static);
        assert_eq!(animated_assignment.backend, WallpaperBackend::Animated);
        assert_eq!(unassigned.backend, WallpaperBackend::Unassigned);
        assert!(registry.compare(&animated_assignment, &animated_assignment));
        assert!(!registry.compare(&static_assignment, &animated_assignment));
    }

    #[test]
    fn generation_does_not_wrap_at_u64_maximum() {
        let identity = monitor("DISPLAY1");
        let mut registry = MonitorAssignmentRegistry {
            current: Default::default(),
            next_generation: [(identity.clone(), u64::MAX)].into_iter().collect(),
        };

        assert!(registry.begin(identity, WallpaperBackend::Static).is_err());
    }

    #[test]
    fn monitor_identity_rejects_empty_values_and_preserves_native_text() {
        assert!(MonitorIdentity::new(" ").is_err());
        let identity = monitor("\\\\.\\DISPLAY1");
        assert_eq!(identity.as_str(), "\\\\.\\DISPLAY1");
    }

    #[test]
    fn assignment_generation_is_not_a_renderer_or_host_identifier() {
        let generation = AssignmentGeneration(7);
        let assignment = MonitorAssignment {
            monitor: monitor("DISPLAY1"),
            backend: WallpaperBackend::Animated,
            generation,
        };
        assert_eq!(assignment.generation, generation);
    }

    fn ui(name: Option<&str>, geometry: MonitorGeometry) -> UiMonitorDescriptor {
        UiMonitorDescriptor {
            name: name.map(str::to_owned),
            geometry,
        }
    }

    fn native(
        device_path: &str,
        source_name: Option<&str>,
        geometry: Option<MonitorGeometry>,
    ) -> NativeMonitorDescriptor {
        NativeMonitorDescriptor {
            device_path: device_path.into(),
            source_name: source_name.map(str::to_owned),
            geometry,
        }
    }

    #[test]
    fn resolves_by_source_name_independently_of_enumeration_order() {
        let geometry_a = MonitorGeometry::new(0, 0, 1920, 1080).unwrap();
        let geometry_b = MonitorGeometry::new(1920, 0, 2560, 1440).unwrap();
        let native = vec![
            native("native-b", Some("\\\\.\\DISPLAY2"), Some(geometry_b)),
            native("native-a", Some("\\\\.\\DISPLAY1"), Some(geometry_a)),
        ];

        assert_eq!(
            resolve_monitor_identity(&ui(Some("\\\\.\\DISPLAY1"), geometry_a), &native),
            Ok(MonitorResolution::Resolved(
                MonitorIdentity::new("native-a").unwrap()
            ))
        );
        assert_eq!(
            resolve_monitor_identity(&ui(Some("\\\\.\\DISPLAY2"), geometry_b), &native),
            Ok(MonitorResolution::Resolved(
                MonitorIdentity::new("native-b").unwrap()
            ))
        );
    }

    #[test]
    fn accepts_negative_physical_coordinates() {
        let geometry = MonitorGeometry::new(-1920, 0, 1920, 1080).unwrap();
        let result = resolve_monitor_identity(
            &ui(None, geometry),
            &[native("native-left", None, Some(geometry))],
        );
        assert_eq!(
            result,
            Ok(MonitorResolution::Resolved(
                MonitorIdentity::new("native-left").unwrap()
            ))
        );
    }

    #[test]
    fn identical_fallback_attributes_are_ambiguous() {
        let geometry = MonitorGeometry::new(0, 0, 1920, 1080).unwrap();
        let result = resolve_monitor_identity(
            &ui(None, geometry),
            &[
                native("native-a", None, Some(geometry)),
                native("native-b", None, Some(geometry)),
            ],
        );
        assert_eq!(result, Ok(MonitorResolution::Ambiguous));
    }

    #[test]
    fn missing_monitor_is_unavailable_without_reassignment() {
        let geometry = MonitorGeometry::new(0, 0, 1920, 1080).unwrap();
        assert_eq!(
            resolve_monitor_identity(&ui(Some("\\\\.\\DISPLAY9"), geometry), &[]),
            Ok(MonitorResolution::Unavailable)
        );
    }

    #[test]
    fn duplicate_native_ids_are_rejected() {
        let geometry = MonitorGeometry::new(0, 0, 1920, 1080).unwrap();
        assert!(resolve_monitor_identity(
            &ui(Some("\\\\.\\DISPLAY1"), geometry),
            &[
                native("same", Some("\\\\.\\DISPLAY1"), Some(geometry)),
                native("same", Some("\\\\.\\DISPLAY2"), Some(geometry)),
            ],
        )
        .is_err());
    }

    #[test]
    fn several_candidates_are_ambiguous() {
        let geometry = MonitorGeometry::new(0, 0, 1920, 1080).unwrap();
        assert_eq!(
            resolve_monitor_identity(
                &ui(Some("\\\\.\\DISPLAY1"), geometry),
                &[
                    native("native-a", Some("\\\\.\\DISPLAY1"), Some(geometry)),
                    native("native-b", Some("\\\\.\\DISPLAY1"), Some(geometry)),
                ],
            ),
            Ok(MonitorResolution::Ambiguous)
        );
    }

    #[test]
    fn invalid_geometry_is_rejected() {
        let geometry = MonitorGeometry {
            x: 0,
            y: 0,
            width: 0,
            height: 1080,
        };
        assert!(resolve_monitor_identity(
            &ui(None, geometry),
            &[native("native-a", None, Some(geometry))],
        )
        .is_err());
    }
}
