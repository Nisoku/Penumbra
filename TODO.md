# Penumbra

- [x] Slint UI

## Slint

- [ ] [Slint](https://slint.dev) for the UI <- write once (or twice since mobile) use anywhere UI.
- [ ] for motion and stuff: Slint `animate` properties with `cubic-bezier` easing + `Timer`s. No separate animation crate needed >:3

- [x] Map canvas v1: infinite pan/zoom plane, chart grid + link edges as merged Slint `Path` layers (the wgpu underlay plan was dropped), viewport culling, LOD, boot camera centered on notes, positions cached in storage.
- [x] Use a Spring Animation in Slint. Cards smoothly drift to their new layout neighbors (drag-release settle, pin, unpin) via a Rust `Timer` easing toward layout targets, overshoot feel per DESIGN.md. (Triggering on embedding stays behind the auto-link pipe.)
- [ ] Camera "Lerp" (Linear Interpolation). If the user hovers over a search result, the camera starts drifting toward it. If they move their mouse away, it drifts back. (Rust Timer driving camera `x`/`y` toward a target that resets on mouse-out)
- [ ] Imagine the user finishes a note on the canvas, and as they hit Esc, the note card slides across the map on its own to snap next to its "relatives" using the auto-associate and link thingy. (card animates `x`/`y` toward its post-edit layout target, MapCanvas emits LayoutChanged)
- [x] Local layout updates only: only the affected neighborhood recalculates (`step_neighborhood`; spatial-hash collision pass)
  - [ ] Atomic Graph Updates: new-note still runs a full `step()`; neighborhood-only trigger waits on the embed/auto-link pipeline
- [x] pinned notes act like fixed stars. They never move and everything else orbits around them. Pin via right-drag (right-tap unpins); left-drag nudges drift back to the pin; pins persist across restarts.
- [x] positions are cached in storage: On startup, the map loads instantly instead of recomputing the entire thing. Throttled save (2s if dirty) + save-on-exit.

## WASM

- [ ] running the Candle inference in a web worker (WASM) or a background thread (Desktop)
- [x] Layout runs in a background worker so the UI thread stays smooth. Physics solves on the tokio runtime off the UI thread (desktop); wasm is still inline pending workers.

## Storage and Sync

- [ ] A built-in Google Drive, GitHub repo, or (limited to 512 MB or 1 GB) Cloud sync
- Storing cache and HNSW index (like how Apple's Spotlight Index is)
- Sync model: Local is the source of truth. Cloud is a mirror.  
  - Sync uploads:
    - notes  
    - metadata  
    - embeddings  
    - layout positions  
    - hnsw index (optional, can be rebuilt)  
  - Sync downloads:
    - same stuff  
    - merge strategy: last write wins for now  
    - if conflicts get weird, the user gets a “choose version” dialog  
- Offline mode: everything works offline, sync just queues changes until the network wakes up.
  - I hope we can integrate vite-plugin-pwa, somehow

## Future

- [ ] Optional encrypted vault mode: keys local. Cloud only sees ciphertext.  
