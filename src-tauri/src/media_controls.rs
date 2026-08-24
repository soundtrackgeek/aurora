use crate::playback::PlaybackSnapshot;
use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use crate::{PlaybackState, playback::PlaybackStatus};
    use std::sync::Mutex;
    use tauri::Manager;
    use windows::{
        Foundation::TypedEventHandler,
        Media::{
            MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
            SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
        },
        Win32::System::WinRT::ISystemMediaTransportControlsInterop,
        core::{HSTRING, factory},
    };

    #[derive(Clone, Copy)]
    enum MediaCommand {
        Play,
        Pause,
        Stop,
        Next,
        Previous,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MetadataProjection {
        track_key: String,
        title: String,
        artist: String,
        album: String,
    }

    pub(crate) struct MediaControlState(Mutex<MediaControls>);

    struct MediaControls {
        controls: SystemMediaTransportControls,
        button_token: i64,
        metadata: Option<MetadataProjection>,
    }

    impl MediaControls {
        fn attach(app: &AppHandle) -> Result<Self, String> {
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| "Aurora's main window is unavailable.".to_owned())?;
            let hwnd = window
                .hwnd()
                .map_err(|error| format!("Aurora could not access its Windows window: {error}"))?;
            let interop: ISystemMediaTransportControlsInterop = factory::<
                SystemMediaTransportControls,
                ISystemMediaTransportControlsInterop,
            >()
            .map_err(|error| format!("Aurora could not open Windows media controls: {error}"))?;
            let controls: SystemMediaTransportControls = unsafe {
                interop.GetForWindow(hwnd).map_err(|error| {
                    format!("Aurora could not bind Windows media controls: {error}")
                })?
            };
            controls.SetIsEnabled(false).map_err(media_control_error)?;
            let handler_app = app.clone();
            let handler = TypedEventHandler::<
                SystemMediaTransportControls,
                SystemMediaTransportControlsButtonPressedEventArgs,
            >::new(move |_, args| {
                let Some(command) = args
                    .as_ref()
                    .and_then(|args| args.Button().ok())
                    .and_then(command_for_button)
                else {
                    return Ok(());
                };
                let command_app = handler_app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    execute_command(&command_app, command)
                });
                Ok(())
            });
            let button_token = controls
                .ButtonPressed(&handler)
                .map_err(media_control_error)?;
            Ok(Self {
                controls,
                button_token,
                metadata: None,
            })
        }

        fn update(&mut self, snapshot: &PlaybackSnapshot) -> Result<(), String> {
            let has_track = snapshot.current_track.is_some();
            self.controls
                .SetIsEnabled(has_track)
                .map_err(media_control_error)?;
            self.controls
                .SetIsPlayEnabled(has_track && snapshot.status != PlaybackStatus::Playing)
                .map_err(media_control_error)?;
            self.controls
                .SetIsPauseEnabled(snapshot.status == PlaybackStatus::Playing)
                .map_err(media_control_error)?;
            self.controls
                .SetIsStopEnabled(has_track)
                .map_err(media_control_error)?;
            self.controls
                .SetIsPreviousEnabled(has_track)
                .map_err(media_control_error)?;
            self.controls
                .SetIsNextEnabled(has_track && snapshot.queue.len() > 1)
                .map_err(media_control_error)?;
            self.controls
                .SetPlaybackStatus(playback_status(snapshot.status))
                .map_err(media_control_error)?;

            let metadata = snapshot
                .current_track
                .as_ref()
                .map(|track| MetadataProjection {
                    track_key: track.track_key.clone(),
                    title: track.title.clone(),
                    artist: track
                        .display_artist
                        .as_deref()
                        .filter(|artist| !artist.trim().is_empty())
                        .unwrap_or(&track.artist)
                        .to_owned(),
                    album: track.album.clone(),
                });
            if metadata == self.metadata {
                return Ok(());
            }
            let updater = self
                .controls
                .DisplayUpdater()
                .map_err(media_control_error)?;
            updater.ClearAll().map_err(media_control_error)?;
            if let Some(metadata) = &metadata {
                updater
                    .SetType(MediaPlaybackType::Music)
                    .map_err(media_control_error)?;
                let properties = updater.MusicProperties().map_err(media_control_error)?;
                properties
                    .SetTitle(&HSTRING::from(&metadata.title))
                    .map_err(media_control_error)?;
                properties
                    .SetArtist(&HSTRING::from(&metadata.artist))
                    .map_err(media_control_error)?;
                properties
                    .SetAlbumTitle(&HSTRING::from(&metadata.album))
                    .map_err(media_control_error)?;
            }
            updater.Update().map_err(media_control_error)?;
            self.metadata = metadata;
            Ok(())
        }

        fn release(&mut self) {
            let _ = self.controls.RemoveButtonPressed(self.button_token);
            let _ = self.controls.SetIsEnabled(false);
        }
    }

    fn command_for_button(button: SystemMediaTransportControlsButton) -> Option<MediaCommand> {
        match button {
            SystemMediaTransportControlsButton::Play => Some(MediaCommand::Play),
            SystemMediaTransportControlsButton::Pause => Some(MediaCommand::Pause),
            SystemMediaTransportControlsButton::Stop => Some(MediaCommand::Stop),
            SystemMediaTransportControlsButton::Next => Some(MediaCommand::Next),
            SystemMediaTransportControlsButton::Previous => Some(MediaCommand::Previous),
            _ => None,
        }
    }

    fn execute_command(app: &AppHandle, command: MediaCommand) {
        let result = {
            let playback = app.state::<PlaybackState>();
            let Ok(mut runtime) = playback.lock() else {
                return;
            };
            match command {
                MediaCommand::Play => runtime.play(),
                MediaCommand::Pause => runtime.pause(),
                MediaCommand::Stop => runtime.stop(),
                MediaCommand::Next => runtime.next(),
                MediaCommand::Previous => runtime.previous(),
            }
        };
        if let Ok(snapshot) = result {
            publish(app, &snapshot);
        }
    }

    fn playback_status(status: PlaybackStatus) -> MediaPlaybackStatus {
        match status {
            PlaybackStatus::Playing => MediaPlaybackStatus::Playing,
            PlaybackStatus::Paused => MediaPlaybackStatus::Paused,
            PlaybackStatus::Stopped | PlaybackStatus::Error => MediaPlaybackStatus::Stopped,
        }
    }

    fn media_control_error(error: windows::core::Error) -> String {
        format!("Windows media controls could not be updated: {error}")
    }

    pub(crate) fn initialize(app: &AppHandle, snapshot: &PlaybackSnapshot) -> Result<(), String> {
        let mut controls = MediaControls::attach(app)?;
        controls.update(snapshot)?;
        app.manage(MediaControlState(Mutex::new(controls)));
        Ok(())
    }

    pub(crate) fn publish(app: &AppHandle, snapshot: &PlaybackSnapshot) {
        let Some(state) = app.try_state::<MediaControlState>() else {
            return;
        };
        if let Ok(mut controls) = state.0.lock() {
            let _ = controls.update(snapshot);
        }
    }

    pub(crate) fn release(app: &AppHandle) {
        let Some(state) = app.try_state::<MediaControlState>() else {
            return;
        };
        if let Ok(mut controls) = state.0.lock() {
            controls.release();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn maps_the_supported_windows_media_buttons() {
            assert!(matches!(
                command_for_button(SystemMediaTransportControlsButton::Play),
                Some(MediaCommand::Play)
            ));
            assert!(matches!(
                command_for_button(SystemMediaTransportControlsButton::Pause),
                Some(MediaCommand::Pause)
            ));
            assert!(matches!(
                command_for_button(SystemMediaTransportControlsButton::Stop),
                Some(MediaCommand::Stop)
            ));
            assert!(matches!(
                command_for_button(SystemMediaTransportControlsButton::Previous),
                Some(MediaCommand::Previous)
            ));
            assert!(matches!(
                command_for_button(SystemMediaTransportControlsButton::Next),
                Some(MediaCommand::Next)
            ));
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub(crate) fn initialize(_: &AppHandle, _: &PlaybackSnapshot) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn publish(_: &AppHandle, _: &PlaybackSnapshot) {}

    pub(crate) fn release(_: &AppHandle) {}
}

pub(crate) use platform::{initialize, publish, release};
