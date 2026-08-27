//! Application orchestrator.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::link::{Link, LinkKind};
use penumbra_core::note::{Note, NoteId};
use penumbra_events::{Event, EventBus};
use penumbra_graph::{GraphSnapshot, GraphStore};
use penumbra_markdown::links::rewrite_wikilink_targets;
use penumbra_storage::{wikilink_targets, Storage};

/// Owns the graph, vault storage, and event bus for one running instance.
pub struct Universe {
    graph: GraphStore,
    storage: Storage,
    events: Arc<EventBus>,
    /// Frontmatter tag subsets per note id, tracked.
    structured_tags: HashMap<NoteId, Vec<String>>,
}

impl Universe {
    /// Open a universe backed by the platform default vault.
    pub async fn open_default() -> Result<Self> {
        let storage = Storage::new().await?;
        Self::open(storage).await
    }

    /// Restore a universe by scanning the given vault.
    ///
    /// Note titles come from filenames and explicit links are derived from
    /// wikilinks in bodies; unresolved targets are skipped.
    pub async fn open(storage: Storage) -> Result<Self> {
        let mut universe = Self {
            graph: GraphStore::new(),
            storage,
            events: Arc::new(EventBus::new()),
            structured_tags: HashMap::new(),
        };
        universe.restore_vault().await?;
        Ok(universe)
    }

    /// Subscribe to domain events emitted by mutations on this universe.
    pub fn events(&self) -> Arc<EventBus> {
        Arc::clone(&self.events)
    }

    /// Read-only access to the restored graph.
    pub fn graph(&self) -> &GraphStore {
        &self.graph
    }

    /// Read-only access to the storage backend.
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Number of notes currently held in memory.
    pub fn note_count(&self) -> usize {
        self.graph.note_count()
    }

    /// Create a note, persist it as a vault file, register the graph node
    /// and its wikilink edges, publish the event.
    pub async fn create_note(&mut self, title: String, body: String) -> Result<NoteId> {
        let note = Note::new(title, body);
        self.storage.save_note(&note, &[]).await?;
        if !self.graph.add_note(note.clone()) {
            return Err(PenumbraError::Graph(format!(
                "note {} already exists",
                note.id
            )));
        }
        self.structured_tags.insert(note.id, Vec::new());
        self.recompute_explicit_links(&note.id);
        let id = note.id;
        self.events.publish(Event::NoteAdded { id, note }).await;
        Ok(id)
    }

    /// Persist a full note update coming from an editor session.
    ///
    /// When the title changed, every other note whose body linked to the
    /// old title is rewritten and republished too, mirroring Obsidian's
    /// rename behavior.
    pub async fn save_note(&mut self, note: Note) -> Result<()> {
        let old_title = match self.graph.get_note(&note.id) {
            Some(existing) => existing.title.clone(),
            None => {
                self.structured_tags.insert(note.id, Vec::new());
                note.title.clone()
            }
        };
        let title_changed = old_title != note.title;

        if self.graph.get_note(&note.id).is_some() {
            self.graph.update_note(&note.id, |existing| {
                *existing = note.clone();
            })?;
        } else if !self.graph.add_note(note.clone()) {
            return Err(PenumbraError::Graph(format!(
                "note {} already exists",
                note.id
            )));
        }

        let tags = self
            .structured_tags
            .get(&note.id)
            .cloned()
            .unwrap_or_default();
        self.storage.save_note(&note, &tags).await?;
        self.recompute_explicit_links(&note.id);

        if title_changed {
            self.propagate_rename(&old_title, &note).await?;
        }

        let id = note.id;
        self.events.publish(Event::NoteUpdated { id, note }).await;
        Ok(())
    }

    /// Remove a note file and its graph node, publish the removal.
    pub async fn delete_note(&mut self, id: &NoteId) -> Result<()> {
        let removed = self
            .graph
            .remove_note(id)
            .ok_or_else(|| PenumbraError::NoteNotFound(id.to_string()))?;
        self.storage.delete_note(id).await?;
        self.structured_tags.remove(id);
        self.events
            .publish(Event::NoteRemoved { id: removed.id })
            .await;
        Ok(())
    }

    async fn restore_vault(&mut self) -> Result<()> {
        let stored = self.storage.scan().await?;
        if stored.is_empty() {
            tracing::info!("empty vault, starting fresh");
            return Ok(());
        }
        let notes: Vec<Note> = stored.iter().map(|item| item.note.clone()).collect();
        for item in &stored {
            self.structured_tags
                .insert(item.note.id, item.fm_tags.clone());
        }
        let links = derive_explicit_links(&notes);
        tracing::info!(
            "restoring universe: {} notes, {} explicit links",
            notes.len(),
            links.len()
        );
        self.graph.restore(GraphSnapshot { notes, links });
        Ok(())
    }

    /// Rewrite inbound `[[old title]]` references across the vault after a
    /// rename and republish updates for every touched note.
    async fn propagate_rename(&mut self, old_title: &str, renamed: &Note) -> Result<()> {
        let affected: Vec<NoteId> = self
            .graph
            .all_notes()
            .filter(|other| other.id != renamed.id)
            .filter(|other| {
                wikilink_targets(&other.body)
                    .iter()
                    .any(|target| target.eq_ignore_ascii_case(old_title))
            })
            .map(|other| other.id)
            .collect();

        for id in affected {
            let mut updated = self
                .graph
                .get_note(&id)
                .expect("collected from graph above")
                .clone();
            updated.body = rewrite_wikilink_targets(&updated.body, old_title, &renamed.title);
            updated.touch();
            self.storage
                .save_note(&updated, &self.structured_tags_for(id))
                .await?;
            self.graph.update_note(&id, |existing| {
                *existing = updated.clone();
            })?;
            let nid = updated.id;
            self.events
                .publish(Event::NoteUpdated {
                    id: nid,
                    note: updated,
                })
                .await;
        }
        Ok(())
    }

    fn structured_tags_for(&self, id: NoteId) -> Vec<String> {
        self.structured_tags.get(&id).cloned().unwrap_or_default()
    }

    /// Replace this note's outgoing Explicit edges with what its body
    /// currently links to.
    fn recompute_explicit_links(&mut self, id: &NoteId) {
        let stale: Vec<(NoteId, NoteId)> = self
            .graph
            .get_links(id)
            .into_iter()
            .filter(|link| link.kind == LinkKind::Explicit && link.source == *id)
            .map(|link| (link.source, link.target))
            .collect();
        for (source, target) in stale {
            if self.graph.unlink_notes(&source, &target).is_err() {
                continue;
            }
        }

        let Some(note) = self.graph.get_note(id).cloned() else {
            return;
        };
        let titles: HashMap<String, NoteId> = self
            .graph
            .all_notes()
            .map(|n| (n.title.to_lowercase(), n.id))
            .collect();
        for target in wikilink_targets(&note.body) {
            let Some(&target_id) = titles.get(&target.to_lowercase()) else {
                continue;
            };
            if target_id == note.id {
                continue;
            }
            // A pre-existing edge (for example an implicit one) wins over
            // adding a duplicate explicit edge in the other direction.
            let _ = self
                .graph
                .link_notes(&note.id, &target_id, LinkKind::Explicit);
        }
    }
}

/// Resolve wikilinks across a set of notes into Explicit links.
///
/// Titles are matched case-insensitively against filenames; the first
/// note claiming a lowercase title wins, matching scan order.
/// TODO: Unresolved targets become ghost nodes once the map supports them.
fn derive_explicit_links(notes: &[Note]) -> Vec<Link> {
    let mut titles: HashMap<String, NoteId> = HashMap::new();
    for note in notes {
        titles.entry(note.title.to_lowercase()).or_insert(note.id);
    }

    let mut seen = HashSet::new();
    let mut links = Vec::new();
    for note in notes {
        for target in wikilink_targets(&note.body) {
            let Some(&target_id) = titles.get(&target.to_lowercase()) else {
                continue;
            };
            if target_id == note.id || !seen.insert((note.id, target_id)) {
                continue;
            }
            links.push(Link::new(note.id, target_id, LinkKind::Explicit));
        }
    }
    links
}
