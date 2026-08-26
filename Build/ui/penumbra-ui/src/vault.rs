//! Vault root resolution.

use opfs::persistent::DirectoryHandle;
use penumbra_core::error::Result;
use penumbra_storage::Storage;

#[cfg(not(target_arch = "wasm32"))]
const CONFIG_DIR: &str = "Penumbra";
#[cfg(not(target_arch = "wasm32"))]
const CONFIG_FILE: &str = "config.json";
#[cfg(not(target_arch = "wasm32"))]
const CONFIG_KEY: &str = "vault_path";
#[cfg(not(target_arch = "wasm32"))]
pub const ENV_VAULT_PATH: &str = "PENUMBRA_VAULT";

#[cfg(not(target_arch = "wasm32"))]
pub async fn resolve_root() -> Result<DirectoryHandle> {
    if let Some(path) = std::env::var_os(ENV_VAULT_PATH) {
        let path = std::path::PathBuf::from(path);
        if path.is_dir() {
            return Ok(DirectoryHandle::from(path));
        }
        tracing::warn!("{} points at a missing directory, ignoring", ENV_VAULT_PATH);
    }

    if let Some(remembered) = remembered_vault_path().await {
        if remembered.is_dir() {
            return Ok(DirectoryHandle::from(remembered));
        }
    }

    if let Some(chosen) = prompt_folder_dialog().await {
        remember_vault_path(&chosen).await;
        return Ok(DirectoryHandle::from(chosen));
    }

    Storage::platform_default_root().await
}

/// The browser build has no folder picker yet, so it always lands on the
/// OPFS default vault; `showDirectoryPicker()` slots in here later.
/// TODO: Add a `prompt_folder_dialog()` for the browser build, and remember path.
#[cfg(target_arch = "wasm32")]
pub async fn resolve_root() -> Result<DirectoryHandle> {
    Storage::platform_default_root().await
}

#[cfg(not(target_arch = "wasm32"))]
async fn remembered_vault_path() -> Option<std::path::PathBuf> {
    use opfs::{
        DirectoryHandle as _, FileHandle as _, GetDirectoryHandleOptions, GetFileHandleOptions,
    };

    let base = opfs::persistent::app_specific_dir().await.ok()?;
    let dir = base
        .get_directory_handle_with_options(CONFIG_DIR, &GetDirectoryHandleOptions { create: false })
        .await
        .ok()?;
    let file = dir
        .get_file_handle_with_options(CONFIG_FILE, &GetFileHandleOptions { create: false })
        .await
        .ok()?;
    let data = file.read().await.ok()?;
    let parsed: serde_json::Value = serde_json::from_slice(&data).ok()?;
    let path = parsed.get(CONFIG_KEY)?.as_str()?.to_string();
    Some(std::path::PathBuf::from(path))
}

#[cfg(not(target_arch = "wasm32"))]
async fn remember_vault_path(path: &std::path::Path) {
    use opfs::{
        CreateWritableOptions, DirectoryHandle as _, FileHandle as _, GetDirectoryHandleOptions,
        GetFileHandleOptions, WritableFileStream as _,
    };

    let write = async {
        let base = opfs::persistent::app_specific_dir().await.ok()?;
        let dir = base
            .get_directory_handle_with_options(
                CONFIG_DIR,
                &GetDirectoryHandleOptions { create: true },
            )
            .await
            .ok()?;
        let mut file = dir
            .get_file_handle_with_options(CONFIG_FILE, &GetFileHandleOptions { create: true })
            .await
            .ok()?;
        let json = serde_json::json!({ CONFIG_KEY: path.to_string_lossy() });
        let mut writer = file
            .create_writable_with_options(&CreateWritableOptions {
                keep_existing_data: false,
            })
            .await
            .ok()?;
        writer
            .write_at_cursor_pos(&json.to_string().into_bytes())
            .await
            .ok()?;
        writer.close().await.ok()?;
        Some(())
    };
    if write.await.is_none() {
        tracing::warn!("could not persist vault choice");
    }
}

/// Show the native folder picker.
///
/// rfd dialogs must be spawned from the thread owning NSApplication on
/// macOS, so the dialog is marshaled onto the Slint UI thread and the
/// result comes back over a channel to whichever task awaited us.
#[cfg(not(target_arch = "wasm32"))]
async fn prompt_folder_dialog() -> Option<std::path::PathBuf> {
    let (sender, receiver) = async_channel::bounded::<Option<std::path::PathBuf>>(1);
    let show = slint::invoke_from_event_loop(move || {
        slint::spawn_local(async move {
            let picked = rfd::AsyncFileDialog::new()
                .set_title("Choose your Penumbra vault")
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf());
            let _ = sender.send(picked).await;
        })
        .expect("slint executor available inside event loop");
    });
    if show.is_err() {
        return None;
    }
    receiver.recv().await.ok().flatten()
}
