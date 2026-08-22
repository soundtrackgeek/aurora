use crate::{
    device_mode::{self, DeviceModeStore, PathMappingStatus},
    state_store::StateStore,
    state_sync::{StartupSyncOutcome, StateMirrorStatus, StateSyncService},
};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaptopModeStatus {
    laptop_mode: bool,
    mode_label: &'static str,
    sync_state: &'static str,
    message: String,
    remote_path: String,
    last_synced_at_ms: Option<i64>,
    mappings: Vec<PathMappingStatus>,
    setting_warning: Option<String>,
}

pub(crate) struct LaptopModeRuntime {
    settings: DeviceModeStore,
    sync: StateSyncService,
}

impl LaptopModeRuntime {
    pub(crate) fn new(
        state_directory: &Path,
        store: StateStore,
        remote_path: PathBuf,
        startup_outcome: StartupSyncOutcome,
    ) -> Result<Self, String> {
        let settings = DeviceModeStore::load(state_directory.join("aurora-device.json"));
        let sync = StateSyncService::new(store, remote_path, startup_outcome)?;
        Ok(Self { settings, sync })
    }

    pub(crate) fn status(&mut self, bypass_throttle: bool) -> LaptopModeStatus {
        let mirror = self.sync.sync_now(bypass_throttle);
        self.combined_status(mirror)
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) -> Result<LaptopModeStatus, String> {
        self.settings.set_laptop_mode(enabled)?;
        Ok(self.status(true))
    }

    fn combined_status(&self, mirror: StateMirrorStatus) -> LaptopModeStatus {
        LaptopModeStatus {
            laptop_mode: self.settings.laptop_mode(),
            mode_label: if self.settings.laptop_mode() {
                "Laptop Mode"
            } else {
                "Desktop Mode"
            },
            sync_state: mirror.sync_state,
            message: mirror.message,
            remote_path: mirror.remote_path,
            last_synced_at_ms: mirror.last_synced_at_ms,
            mappings: device_mode::path_mapping_statuses(),
            setting_warning: self.settings.warning().map(str::to_owned),
        }
    }
}
