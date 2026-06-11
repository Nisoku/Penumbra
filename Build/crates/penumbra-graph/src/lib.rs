use std::collections::{HashMap, HashSet};

use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::link::{Link, LinkKind};
use penumbra_core::note::{Note, NoteId};
use petgraph::graph::{NodeIndex, UnGraph};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub notes: Vec<Note>,
    pub links: Vec<Link>,
}

pub struct GraphStore {
    notes: HashMap<NoteId, Note>,
    graph: UnGraph<NoteId, Link>,
    node_index: HashMap<NoteId, NodeIndex>,
}

impl GraphStore {
    pub fn new() -> Self {
        Self {
            notes: HashMap::new(),
            graph: UnGraph::new_undirected(),
            node_index: HashMap::new(),
        }
    }

    pub fn add_note(&mut self, note: Note) -> bool {
        let id = note.id;
        if self.notes.contains_key(&id) {
            return false;
        }
        let idx = self.graph.add_node(id);
        self.node_index.insert(id, idx);
        self.notes.insert(id, note);
        true
    }

    pub fn remove_note(&mut self, id: &NoteId) -> Option<Note> {
        let note = self.notes.remove(id)?;
        if let Some(&idx) = self.node_index.get(id) {
            let neighbor_ids: Vec<NoteId> = self
                .graph
                .neighbors(idx)
                .map(|n| self.graph[n].clone())
                .collect();
            for neighbor_id in &neighbor_ids {
                if let Some(&neighbor_idx) = self.node_index.get(neighbor_id) {
                    self.graph.remove_edge(
                        self.graph
                            .find_edge(idx, neighbor_idx)
                            .expect("edge should exist"),
                    );
                }
            }
            self.graph.remove_node(idx);
            self.node_index.remove(id);
        }
        Some(note)
    }

    pub fn get_note(&self, id: &NoteId) -> Option<&Note> {
        self.notes.get(id)
    }

    pub fn get_note_mut(&mut self, id: &NoteId) -> Option<&mut Note> {
        self.notes.get_mut(id)
    }

    pub fn update_note(&mut self, id: &NoteId, f: impl FnOnce(&mut Note)) -> Result<()> {
        let note = self.notes.get_mut(id).ok_or_else(|| {
            PenumbraError::NoteNotFound(id.to_string())
        })?;
        f(note);
        note.touch();
        Ok(())
    }

    pub fn link_notes(
        &mut self,
        source: &NoteId,
        target: &NoteId,
        kind: LinkKind,
    ) -> Result<Link> {
        if !self.notes.contains_key(source) {
            return Err(PenumbraError::NoteNotFound(source.to_string()));
        }
        if !self.notes.contains_key(target) {
            return Err(PenumbraError::NoteNotFound(target.to_string()));
        }

        let source_idx = self.node_index[source];
        let target_idx = self.node_index[target];

        if self.graph.find_edge(source_idx, target_idx).is_some() {
            return Err(PenumbraError::Graph(format!(
                "link already exists between {} and {}",
                source, target
            )));
        }

        let link = Link::new(*source, *target, kind);
        self.graph.add_edge(source_idx, target_idx, link.clone());
        Ok(link)
    }

    pub fn unlink_notes(&mut self, source: &NoteId, target: &NoteId) -> Result<Link> {
        let source_idx = self.node_index.get(source).ok_or_else(|| {
            PenumbraError::NoteNotFound(source.to_string())
        })?;
        let target_idx = self.node_index.get(target).ok_or_else(|| {
            PenumbraError::NoteNotFound(target.to_string())
        })?;

        let edge = self
            .graph
            .find_edge(*source_idx, *target_idx)
            .ok_or_else(|| {
                PenumbraError::Graph(format!(
                    "no link between {} and {}",
                    source, target
                ))
            })?;

        let link = self.graph.remove_edge(edge).unwrap();
        Ok(link)
    }

    pub fn get_neighbors(&self, id: &NoteId) -> Vec<&Note> {
        let idx = match self.node_index.get(id) {
            Some(&idx) => idx,
            None => return Vec::new(),
        };
        self.graph
            .neighbors(idx)
            .filter_map(|n| {
                let neighbor_id = &self.graph[n];
                self.notes.get(neighbor_id)
            })
            .collect()
    }

    pub fn get_links(&self, id: &NoteId) -> Vec<&Link> {
        let idx = match self.node_index.get(id) {
            Some(&idx) => idx,
            None => return Vec::new(),
        };
        self.graph
            .edges(idx)
            .map(|e| e.weight())
            .collect()
    }

    pub fn get_connected_component(&self, id: &NoteId) -> HashSet<NoteId> {
        let idx = match self.node_index.get(id) {
            Some(&idx) => idx,
            None => return HashSet::new(),
        };
        let mut component = HashSet::new();
        let mut stack = vec![idx];
        while let Some(current) = stack.pop() {
            let current_id = &self.graph[current];
            if !component.insert(*current_id) {
                continue;
            }
            for neighbor in self.graph.neighbors(current) {
                let neighbor_id = &self.graph[neighbor];
                if !component.contains(neighbor_id) {
                    stack.push(neighbor);
                }
            }
        }
        component
    }

    pub fn all_notes(&self) -> impl Iterator<Item = &Note> + '_ {
        self.notes.values()
    }

    pub fn all_links(&self) -> Vec<&Link> {
        self.graph
            .edge_indices()
            .map(|e| &self.graph[e])
            .collect()
    }

    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    pub fn link_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            notes: self.notes.values().cloned().collect(),
            links: self.all_links().into_iter().cloned().collect(),
        }
    }

    pub fn restore(&mut self, snapshot: GraphSnapshot) {
        self.notes.clear();
        self.graph = UnGraph::new_undirected();
        self.node_index.clear();

        for note in snapshot.notes {
            self.add_note(note);
        }
        for link in snapshot.links {
            let source_idx = self.node_index[&link.source];
            let target_idx = self.node_index[&link.target];
            self.graph.add_edge(source_idx, target_idx, link);
        }
    }
}

impl Default for GraphStore {
    fn default() -> Self {
        Self::new()
    }
}
