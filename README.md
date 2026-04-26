# Penumbra
A spatial notes app, made with true cross-platformity in mind.

<!-- Started this like 8:02 AM 4/19 -->

## random things

- Imagine the user finishes a note on the canvas, and as they hit Esc, the note card slides across the map on its own to snap next to its "relatives" using the auto-associate and link thingy.
  - Use a Spring Animation in Dioxus. When the Candle calculation finishes, just update the "Target Coordinates" and let the card smoothly drift to its new neighbors
- running the Candle inference in a web worker (WASM) or a background thread (Desktop)
- Camera "Lerp" (Linear Interpolation). If the user hovers over a search result, the camera starts drifting toward it. If they move their mouse away, it drifts back. 

## Stuff we're using

- UI:
  - [Dioxus](https://github.com/dioxuslabs/dioxus) for the UI <- write once (or twice since mobile) use anywhere UI
  - for motion and stuff: [dioxus-motion](https://github.com/wheregmis/dioxus-motion)

- Layout Engine:
  - Powered by:
    - Graph Logic: [petgraph](https://github.com/petgraph/petgraph)
    - Map Layout: [vibe_graph_layout_gpu](https://github.com/pinsky-three/vibe-graph) (sigh that name :/ i'm still using it tho :P)
  - Using a ForceAtlas2 style layout. Notes repel each other; related notes pull together. 
  - Barnes-Hut optimization so the whole thing doesn’t melt your CPU once you hit like 2k notes. Distant nodes get grouped into “supernodes” so the physics stays fast.
  - local updates only: If one note changes, only its neighborhood recalculates. No full‑graph “everything explodes and re‑settles” nonsense.
  - pinned notes act like fixed stars. They never move and everything else orbits around them. but like you DON'T need to have them either, natural ones via auto-linking (or manual linking) exist as well
  - Layout runs in a background worker so the UI thread stays smooth. The map should never stutter because physics is cooking.
  - positions are cached in storage: On startup, the map loads instantly instead of recomputing the entire thing.
  - smooth drift: when a note’s target position changes (new embedding, new links, new tags), the UI doesn’t teleport it. It just updates the target and lets the spring animation glide it into place.
  - zoom‑aware LOD: when zoomed out, the layout engine doesn’t care about tiny card geometry. It treats notes as points. When zoomed in, it respects card sizes so things don’t overlap.
  - collision avoidance: cards shouldn’t sit on top of each other but they shouldn't like take up like massive amounts of space either it's like tiny preview cards (ooh this would be a good use of AI (wait no Apple Notification Summaries 😭 maybe not)) 


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
  - Use [usearch](https://github.com/unum-cloud/USearch) for the index
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
  - Vector Search: run search queries through usearch as well
    - using [usearch](https://github.com/unum-cloud/USearch)
  - Hybrid search combines:
    - exact text match
    - semantic similarity
    - tag filters
  - Results guide the map (pan/zoom to relevant clusters)
    - smooth map move when you hover over a search result to the specific note

- Storage:
  - A built-in GDrive/Cloud sync
  - [OPFS crate](https://github.com/anchpop/opfs) <- write once use anywhere storage
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
  - Optional encrypted vault mode: keys local. Cloud only sees ciphertext.  
    - very much a future thing tho

<!-- stopped 12:10 pm -->

<!-- continuing 2:05 pm -->
