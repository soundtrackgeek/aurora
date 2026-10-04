use crate::{
    device_mode::{self, DeviceModeStore, PathMappingStatus},
    state_store::StateStore,
    state_sync::{StartupSyncOutcome, StateMirrorStatus, StateSyncService},
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaptopModeStatus {
    laptop_mode: bool,
    mode_label: &'static str,
    sync_state: &'static str,
    message: String,
    remote_path: String,
    last_synced_at_ms: Option<i64>,
    album_order_revision: u64,
    mappings: Vec<PathMappingStatus>,
    setting_warning: Option<String>,
}

pub(crate) struct LaptopModeRuntime {
    settings: DeviceModeStore,
    sync: StateSyncService,
    mirror: StateMirrorStatus,
}

impl LaptopModeRuntime {
    pub(crate) fn new(
        settings: DeviceModeStore,
        store: StateStore,
        remote_path: PathBuf,
        startup_outcome: StartupSyncOutcome,
    ) -> Result<Self, String> {
        let sync = StateSyncService::new(store, remote_path.clone(), startup_outcome)?;
        let mirror = StateMirrorStatus {
            sync_state: "pending",
            message: "Waiting for the native sync monitor.".to_owned(),
            remote_path: remote_path.to_string_lossy().into_owned(),
            last_synced_at_ms: None,
            album_order_revision: 0,
        };
        Ok(Self {
            settings,
            sync,
            mirror,
        })
    }

    pub(crate) fn status(&mut self, bypass_throttle: bool) -> LaptopModeStatus {
        self.mirror = self.sync.sync_now(bypass_throttle);
        self.cached_status()
    }

    pub(crate) fn cached_status(&self) -> LaptopModeStatus {
        self.combined_status(self.mirror.clone())
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) -> Result<LaptopModeStatus, String> {
        if crate::connections::network_mode() {
            return Err(
                "Change Network Mode in Settings → Connections, then restart Aurora.".to_owned(),
            );
        }
        self.settings.set_laptop_mode(enabled)?;
        Ok(self.status(true))
    }

    fn combined_status(&self, mirror: StateMirrorStatus) -> LaptopModeStatus {
        LaptopModeStatus {
            laptop_mode: self.settings.laptop_mode(),
            mode_label: if crate::connections::network_mode() {
                "Network Mode"
            } else if self.settings.laptop_mode() {
                "Laptop Mode"
            } else {
                "Desktop Mode"
            },
            sync_state: mirror.sync_state,
            message: mirror.message,
            remote_path: mirror.remote_path,
            last_synced_at_ms: mirror.last_synced_at_ms,
            album_order_revision: mirror.album_order_revision,
            mappings: device_mode::path_mapping_statuses(),
            setting_warning: self.settings.warning().map(str::to_owned),
        }
    }
}
