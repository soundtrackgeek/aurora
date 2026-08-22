use crate::state_sync;
use cpal::{
    Device, DeviceId,
    traits::{DeviceTrait, HostTrait},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

const SETTINGS_VERSION: u8 = 1;
pub(crate) const SYSTEM_DEFAULT_DEVICE_ID: &str = "system-default";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReplayGainMode {
    #[default]
    Off,
    Track,
    Album,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AudioSettings {
    pub(crate) output_device_id: String,
    pub(crate) replay_gain_mode: ReplayGainMode,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            output_device_id: SYSTEM_DEFAULT_DEVICE_ID.to_owned(),
            replay_gain_mode: ReplayGainMode::Off,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AudioSettingsRequest {
    pub(crate) output_device_id: String,
    pub(crate) replay_gain_mode: ReplayGainMode,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AudioSettingsFile {
    version: u8,
    settings: AudioSettings,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioOutputDevice {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) is_default: bool,
}

pub(crate) struct DiscoveredOutputDevice {
    pub(crate) info: AudioOutputDevice,
    pub(crate) device: Device,
}

pub(crate) struct SelectedOutputDevice {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) device: Device,
    pub(crate) using_fallback: bool,
    pub(crate) message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioSettingsStatus {
    pub(crate) settings: AudioSettings,
    pub(crate) devices: Vec<AudioOutputDevice>,
    pub(crate) active_device_id: Option<String>,
    pub(crate) active_device_label: Option<String>,
    pub(crate) using_fallback: bool,
    pub(crate) message: Option<String>,
    pub(crate) error: Option<String>,
}

pub(crate) struct AudioSettingsStore {
    path: PathBuf,
    settings: AudioSettings,
    warning: Option<String>,
}

impl AudioSettingsStore {
    pub(crate) fn load(path: PathBuf) -> Self {
        let mut warning = None;
        let settings = if path.is_file() {
            match fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<AudioSettingsFile>(&json)
                        .map_err(|error| error.to_string())
                }) {
                Ok(file) if file.version == SETTINGS_VERSION => file.settings,
                Ok(_) => {
                    warning = Some(
                        "Aurora found audio settings from an unsupported version and used safe defaults."
                            .to_owned(),
                    );
                    AudioSettings::default()
                }
                Err(error) => {
                    warning = Some(format!(
                        "Aurora could not read this device's audio settings and used safe defaults: {error}"
                    ));
                    AudioSettings::default()
                }
            }
        } else {
            AudioSettings::default()
        };
        Self {
            path,
            settings,
            warning,
        }
    }

    pub(crate) fn settings(&self) -> &AudioSettings {
        &self.settings
    }

    pub(crate) fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    pub(crate) fn update(&mut self, request: AudioSettingsRequest) -> Result<(), String> {
        validate_device_id(&request.output_device_id)?;
        let previous = self.settings.clone();
        self.settings = AudioSettings {
            output_device_id: request.output_device_id,
            replay_gain_mode: request.replay_gain_mode,
        };
        if let Err(error) = self.persist() {
            self.settings = previous;
            return Err(error);
        }
        self.warning = None;
        Ok(())
    }

    fn persist(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Aurora's audio setting has no parent directory.".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create Aurora's audio settings folder: {error}"))?;
        let json = serde_json::to_vec_pretty(&AudioSettingsFile {
            version: SETTINGS_VERSION,
            settings: self.settings.clone(),
        })
        .map_err(|error| format!("Could not encode Aurora's audio settings: {error}"))?;
        let temporary = parent.join(format!(
            ".aurora-audio-{}-{}.tmp",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut file = File::options()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("Could not save Aurora's audio settings: {error}"))?;
        if let Err(error) = file.write_all(&json).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(format!("Could not flush Aurora's audio settings: {error}"));
        }
        drop(file);
        let result = if self.path.is_file() {
            state_sync::replace_file_atomic(&self.path, &temporary)
        } else {
            fs::rename(&temporary, &self.path)
                .map_err(|error| format!("Could not install Aurora's audio settings: {error}"))
        };
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

pub(crate) fn discover_output_devices() -> Result<Vec<DiscoveredOutputDevice>, String> {
    let host = cpal::default_host();
    let default_id = host
        .default_output_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());
    let devices = host
        .output_devices()
        .map_err(|error| format!("Windows could not enumerate audio outputs: {error}"))?;
    let mut discovered = Vec::new();
    for device in devices {
        let Ok(id) = device.id().map(|id| id.to_string()) else {
            continue;
        };
        let label = device
            .description()
            .map(|description| description.name().to_owned())
            .unwrap_or_else(|_| "Windows audio output".to_owned());
        discovered.push(DiscoveredOutputDevice {
            info: AudioOutputDevice {
                is_default: default_id.as_deref() == Some(id.as_str()),
                id,
                label,
            },
            device,
        });
    }
    discovered.sort_by(|left, right| {
        right
            .info
            .is_default
            .cmp(&left.info.is_default)
            .then_with(|| left.info.label.cmp(&right.info.label))
    });
    Ok(discovered)
}

pub(crate) fn select_output_device(
    requested_device_id: &str,
    force_system_default: bool,
) -> Result<SelectedOutputDevice, String> {
    let discovered = discover_output_devices()?;
    if discovered.is_empty() {
        return Err("Windows did not report an available audio output.".to_owned());
    }
    let wants_default = force_system_default || requested_device_id == SYSTEM_DEFAULT_DEVICE_ID;
    let requested = (!wants_default).then_some(requested_device_id);
    let selected = requested
        .and_then(|id| discovered.iter().find(|candidate| candidate.info.id == id))
        .or_else(|| {
            discovered
                .iter()
                .find(|candidate| candidate.info.is_default)
        })
        .unwrap_or(&discovered[0]);
    let using_fallback = force_system_default || requested.is_some_and(|id| selected.info.id != id);
    let message = if force_system_default {
        Some(format!(
            "The selected output stopped responding. Aurora continued on {}.",
            selected.info.label
        ))
    } else if using_fallback {
        Some(format!(
            "The selected output is unavailable. Aurora continued on {} without changing your preference.",
            selected.info.label
        ))
    } else {
        None
    };
    Ok(SelectedOutputDevice {
        id: selected.info.id.clone(),
        label: selected.info.label.clone(),
        device: selected.device.clone(),
        using_fallback,
        message,
    })
}

fn validate_device_id(value: &str) -> Result<(), String> {
    if value == SYSTEM_DEFAULT_DEVICE_ID {
        return Ok(());
    }
    if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err("Choose a valid Windows audio output.".to_owned());
    }
    DeviceId::from_str(value)
        .map(|_| ())
        .map_err(|_| "Choose a valid Windows audio output.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_device_local() {
        let settings = AudioSettings::default();
        assert_eq!(settings.output_device_id, SYSTEM_DEFAULT_DEVICE_ID);
        assert_eq!(settings.replay_gain_mode, ReplayGainMode::Off);
    }

    #[test]
    fn invalid_device_ids_are_rejected() {
        assert!(validate_device_id("").is_err());
        assert!(validate_device_id("not-a-cpal-device").is_err());
        assert!(validate_device_id(SYSTEM_DEFAULT_DEVICE_ID).is_ok());
    }

    #[test]
    #[ignore = "requires an interactive Windows audio session"]
    fn discovers_stable_windows_output_ids() {
        let devices = discover_output_devices().expect("enumerate Windows outputs");
        assert!(!devices.is_empty());
        assert!(devices.iter().any(|device| device.info.is_default));
        for device in devices {
            DeviceId::from_str(&device.info.id).expect("round-trip stable CPAL device ID");
            assert!(!device.info.label.trim().is_empty());
        }
    }
}
