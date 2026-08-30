use std::future::Future;
use std::sync::OnceLock;

use opfs::persistent::DirectoryHandle;
use penumbra_core::error::Result;
use penumbra_storage::Storage;
use tokio::runtime::Runtime;

const CONFIG_DIR: &str = "Penumbra";
const CONFIG_FILE: &str = "config.json";
const CONFIG_KEY: &str = "vault_path";
const ENV_VAULT_PATH: &str = "PENUMBRA_VAULT";

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build the tokio runtime")
    })
}

/// Run a future to completion on the UI thread with a tokio reactor backing
/// file I/O.
pub fn spawn(future: impl Future<Output = ()> + 'static) {
    runtime().block_on(future);
}

/// Run a future to completion on the UI thread after the loop has stopped.
pub fn block_on<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

pub async fn vault_root() -> Result<DirectoryHandle> {
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

    if let Some(chosen) = prompt_folder_dialog() {
        remember_vault_path(&chosen).await;
        return Ok(DirectoryHandle::from(chosen));
    }

    Storage::platform_default_root().await
}

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

/// Modal folder picker
fn prompt_folder_dialog() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choose your Penumbra vault")
        .pick_folder()
}
