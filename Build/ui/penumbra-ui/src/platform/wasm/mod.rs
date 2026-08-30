use std::future::Future;

use opfs::persistent::DirectoryHandle;
use penumbra_core::error::Result;
use penumbra_storage::Storage;
use wasm_bindgen::prelude::wasm_bindgen;

/// The browser build has no folder picker yet, so it always lands on the
/// OPFS default vault; `showDirectoryPicker()` goes in here later.
pub async fn vault_root() -> Result<DirectoryHandle> {
    Storage::platform_default_root().await
}

/// Run a future on the browser event loop.
pub fn spawn(future: impl Future<Output = ()> + 'static) {
    let _ = slint::spawn_local(future);
}

/// Run a future to completion
pub fn block_on<F: Future>(future: F) -> F::Output {
    futures::executor::block_on(future)
}

/// Browser entry point invoked by `wasm-bindgen` when the module loads.
#[wasm_bindgen(start)]
pub fn boot() {
    crate::start_app().expect("penumbra failed to start");
}
