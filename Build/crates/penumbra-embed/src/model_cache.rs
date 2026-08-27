//! Cross-platform model byte caching.

/// Retrieve cached model bytes by logical name.
pub async fn get(name: &str) -> Option<Vec<u8>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        get_native(name)
    }
    #[cfg(target_arch = "wasm32")]
    {
        get_wasm(name).await
    }
}

/// Store model bytes under a logical name.
pub async fn put(name: &str, data: &[u8]) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        put_native(name, data)
    }
    #[cfg(target_arch = "wasm32")]
    {
        put_wasm(name, data).await
    }
}

const CACHE_DIR: &str = "penumbra-models";

#[cfg(not(target_arch = "wasm32"))]
fn cache_root() -> std::path::PathBuf {
    std::env::temp_dir().join(CACHE_DIR)
}

#[cfg(not(target_arch = "wasm32"))]
fn get_native(name: &str) -> Option<Vec<u8>> {
    std::fs::read(cache_root().join(name)).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn put_native(name: &str, data: &[u8]) {
    let root = cache_root();
    let _ = std::fs::create_dir_all(&root);
    let _ = std::fs::write(root.join(name), data);
}

#[cfg(target_arch = "wasm32")]
async fn get_wasm(name: &str) -> Option<Vec<u8>> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window()?;
    let caches = window.caches().ok()?;

    let cache_val = wasm_bindgen_futures::JsFuture::from(caches.open(CACHE_DIR))
        .await
        .ok()?;
    let cache: web_sys::Cache = cache_val.dyn_into().ok()?;

    let req_init = web_sys::RequestInit::new();
    req_init.set_method("GET");
    let request = web_sys::Request::new_with_str_and_init(name, &req_init).ok()?;

    let match_val = wasm_bindgen_futures::JsFuture::from(cache.match_with_request(&request))
        .await
        .ok()?;
    let response: web_sys::Response = match_val.dyn_into().ok()?;

    let promise = response.array_buffer().ok()?;
    let array_buf = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
    let uint8 = js_sys::Uint8Array::new(&array_buf);
    Some(uint8.to_vec())
}

#[cfg(target_arch = "wasm32")]
async fn put_wasm(name: &str, data: &[u8]) {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(caches) = window.caches() else {
        return;
    };

    let cache_val = match wasm_bindgen_futures::JsFuture::from(caches.open(CACHE_DIR)).await {
        Ok(v) => v,
        Err(_) => return,
    };
    let cache: web_sys::Cache = match cache_val.dyn_into() {
        Ok(c) => c,
        Err(_) => return,
    };

    let uint8 = js_sys::Uint8Array::from(data);
    let resp_init = web_sys::ResponseInit::new();
    resp_init.set_status(200);
    let body = match web_sys::Response::new_with_opt_buffer_source_and_init(
        Some(&uint8.into()),
        &resp_init,
    ) {
        Ok(r) => r,
        Err(_) => return,
    };

    let req_init = web_sys::RequestInit::new();
    req_init.set_method("PUT");
    let request = match web_sys::Request::new_with_str_and_init(name, &req_init) {
        Ok(r) => r,
        Err(_) => return,
    };

    let _ = cache.put_with_request(&request, &body);
}
