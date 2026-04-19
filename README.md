# Penumbra
A spatial notes app, made with true cross-platformity in mind.

<!-- Started this like 8:02 AM 4/19 -->

## random things

- Imagine the user finishes a note on the canvas, and as they hit Esc, the note card slides across the map on its own to snap next to its "relatives" using the auto-associate and link thingy.
  - Use a Spring Animation in Dioxus. When the Candle calculation finishes, just update the "Target Coordinates" and let the card smoothly drift to its new neighbors
- running the Candle inference in a web worker (WASM) or a background thread (Desktop)
- Camera "Lerp" (Linear Interpolation). If the user hovers over a search result, the camera starts drifting toward it. If they move their mouse away, it drifts back.
- 

## Stuff we're using

- [Dioxus](https://github.com/dioxuslabs/dioxus) for the UI <- write once (or twice since mobile) use anywhere UI

- serde/serde_json//wasm_bindgen (if needed)/tokio (all WASM compat no non wasm compat stuff)
- uuid (notes stuff ofc)

- Markdown Parsing, Rendering, etc:
  - Core Parser: [pulldown‑cmark](https://github.com/pulldown-cmark/pulldown-cmark)
  - Handling the live input cleanly and streaming, etc: [mdstream](https://github.com/Latias94/mdstream)
  - Safety/Sanitization: [Ammonia](https://github.com/rust-ammonia/ammonia)
  - MD to Dioxus: custom based on Dioxus's [official one](https://github.com/DioxusLabs/markdown)
  - Custom Syntax:
    - A) mdstream preprocessors
    - B) pulldown event filters
    - C) Fork Pulldown (if it's like too difficult/annoying to do manually)

- NLP and Auto-Linking:
  - Use [hnsw_rs](https://github.com/Gumo-A/hnsw_rs) (sub‑millisecond nearest neighbor search)
  - Use [candle](https://github.com/huggingface/candle)
  - Model: [Snowflake-Arctic-Embed-XS](https://huggingface.co/Snowflake/snowflake-arctic-embed-xs)
    - probably quantized for WASM [here](https://huggingface.co/ChristianAzinn/snowflake-arctic-embed-xs-gguf/)
      -  Choosable which one by user, probably offer original unquantized model as well (with performance warnings haha)
  - the user-defined tags take priority, duh
  - explicit links outrank implicit ones, duh
  - the auto-linking fills in the gaps
    - represented differently in UI
  -  Temporal Weighting (older notes less relevant)
  - Pinnable cards that can never move
  - Local layout updates only: only the affected neighborhood recalculates
    -  Atomic Graph Updates: (when I have a new note get analyzed, it shouldn't update everything, it should atomically only update one area)

- Searching:
  - Vector Search: run search queries through the candle model to get a better search for free
  - Hybrid search combines:
    - exact text match
    - semantic similarity
    - tag filters
  - Results guide the map (pan/zoom to relevant clusters)
    - smooth map move when you hover over a search result to the specific note

- Storage:
 - A built-in GDrive/Cloud sync
 - [OPFS crate](https://github.com/anchpop/opfs) <- write once use anywhere storage

[continue] [stopped 12:10 pm]
