# Penumbra

- [ ] Dioxus UI

## Dioxus

- [Dioxus](https://github.com/dioxuslabs/dioxus) for the UI <- write once (or twice since mobile) use anywhere UI
- for motion and stuff: [dioxus-motion](https://github.com/wheregmis/dioxus-motion)

- [ ] Use a Spring Animation in Dioxus. When the Candle calculation finishes, just update the "Target Coordinates" and let the card smoothly drift to its new neighbors
- [ ] Camera "Lerp" (Linear Interpolation). If the user hovers over a search result, the camera starts drifting toward it. If they move their mouse away, it drifts back.
- [ ] Imagine the user finishes a note on the canvas, and as they hit Esc, the note card slides across the map on its own to snap next to its "relatives" using the auto-associate and link thingy.
- [ ] Local layout updates only: only the affected neighborhood recalculates
  - Atomic Graph Updates: (when I have a new note get analyzed, it shouldn't update everything, it should atomically only update one area)
- [ ] pinned notes act like fixed stars. They never move and everything else orbits around them. but like you DON'T need to have them either, natural ones via auto-linking (or manual linking) exist as well
- [ ] positions are cached in storage: On startup, the map loads instantly instead of recomputing the entire thing.

## WASM

- [ ] running the Candle inference in a web worker (WASM) or a background thread (Desktop)
- [ ] Layout runs in a background worker so the UI thread stays smooth. The map should never stutter because physics is cooking.

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
