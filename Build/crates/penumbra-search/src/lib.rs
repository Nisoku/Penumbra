use std::sync::Arc;

use chrono::{DateTime, Utc};
use penumbra_core::embed::EmbeddingProvider;
use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::note::{Note, NoteId};
use penumbra_index::VectorIndex;

pub struct SearchConfig {
    pub vector_weight: f64,
    pub text_weight: f64,
    pub tag_weight: f64,
    pub temporal_decay_days: f64,
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            vector_weight: 1.0,
            text_weight: 0.5,
            tag_weight: 0.3,
            temporal_decay_days: 365.0,
            max_results: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub note_id: NoteId,
    pub note: Note,
    pub score: f64,
    pub vector_score: f64,
    pub text_score: f64,
    pub tag_score: f64,
    pub temporal_score: f64,
}

pub struct SearchEngine {
    embedder: Arc<dyn EmbeddingProvider>,
    index: Arc<std::sync::Mutex<dyn VectorIndex>>,
    config: SearchConfig,
}

impl SearchEngine {
    pub fn new(
        embedder: Arc<dyn EmbeddingProvider>,
        index: Arc<std::sync::Mutex<dyn VectorIndex>>,
    ) -> Self {
        Self {
            embedder,
            index,
            config: SearchConfig::default(),
        }
    }

    pub fn with_config(
        embedder: Arc<dyn EmbeddingProvider>,
        index: Arc<std::sync::Mutex<dyn VectorIndex>>,
        config: SearchConfig,
    ) -> Self {
        Self {
            embedder,
            index,
            config,
        }
    }

    pub fn embedder(&self) -> &Arc<dyn EmbeddingProvider> {
        &self.embedder
    }

    pub fn index(&self) -> &Arc<std::sync::Mutex<dyn VectorIndex>> {
        &self.index
    }

    pub fn config(&self) -> &SearchConfig {
        &self.config
    }

    pub async fn search(
        &self,
        query: &str,
        notes: &[Note],
        tags: &[String],
    ) -> Result<Vec<SearchResult>> {
        if query.is_empty() && tags.is_empty() {
            return Ok(Vec::new());
        }

        let tag_filter: Option<Vec<NoteId>> = if tags.is_empty() {
            None
        } else {
            Some(
                notes
                    .iter()
                    .filter(|n| tags.iter().all(|t| n.tags.iter().any(|nt| nt == t)))
                    .map(|n| n.id)
                    .collect(),
            )
        };

        let query_lower = query.to_lowercase();
        let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

        let note_map: std::collections::HashMap<NoteId, &Note> =
            notes.iter().map(|n| (n.id, n)).collect();

        if !query.is_empty() {
            let embedding = self
                .embedder
                .embed_text(query)
                .await
                .map_err(|e| PenumbraError::Search(format!("embedding: {e}")))?;

            let index = self.index.lock().unwrap();
            let index_hits = index
                .search(&embedding, self.config.max_results * 2)
                .map_err(|e| PenumbraError::Search(format!("index search: {e}")))?;

            let now = Utc::now();
            let mut results: Vec<SearchResult> = index_hits
                .into_iter()
                .filter_map(|hit| {
                    let note = note_map.get(&hit.id)?;

                    if let Some(ref allowed) = tag_filter {
                        if !allowed.contains(&hit.id) {
                            return None;
                        }
                    }

                    let text_score = text_relevance(&query_tokens, note);
                    let tag_score = tag_relevance(tags, note);
                    let temporal_score = temporal_weight(
                        &note.meta.updated_at,
                        now,
                        self.config.temporal_decay_days,
                    );

                    let vector_score = hit.score.max(0.0) as f64;

                    let score = vector_score * self.config.vector_weight
                        + text_score * self.config.text_weight
                        + tag_score * self.config.tag_weight
                        + temporal_score;

                    Some(SearchResult {
                        note_id: hit.id,
                        note: (**note).clone(),
                        score,
                        vector_score,
                        text_score,
                        tag_score,
                        temporal_score,
                    })
                })
                .collect();

            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(self.config.max_results);
            Ok(results)
        } else if let Some(allowed) = tag_filter {
            let now = Utc::now();
            let mut results: Vec<SearchResult> = allowed
                .iter()
                .filter_map(|id| {
                    let note = note_map.get(id)?;
                    let text_score = text_relevance(&query_tokens, note);
                    let tag_score = tag_relevance(tags, note);
                    let temporal_score = temporal_weight(
                        &note.meta.updated_at,
                        now,
                        self.config.temporal_decay_days,
                    );

                    Some(SearchResult {
                        note_id: *id,
                        note: (*note).clone(),
                        score: text_score * self.config.text_weight
                            + tag_score * self.config.tag_weight
                            + temporal_score,
                        vector_score: 0.0,
                        text_score,
                        tag_score,
                        temporal_score,
                    })
                })
                .collect();

            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(self.config.max_results);
            Ok(results)
        } else {
            Ok(Vec::new())
        }
    }
}

fn text_relevance(query_tokens: &[&str], note: &Note) -> f64 {
    if query_tokens.is_empty() {
        return 0.0;
    }

    let title_lower = note.title.to_lowercase();
    let body_lower = note.body.to_lowercase();

    let mut hits = 0usize;
    for token in query_tokens {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric());
        if token.is_empty() {
            continue;
        }
        if title_lower.contains(token) {
            hits += 3;
        } else if body_lower.contains(token) {
            hits += 1;
        }
    }

    hits as f64 / (query_tokens.len() as f64 * 3.0)
}

fn tag_relevance(query_tags: &[String], note: &Note) -> f64 {
    if query_tags.is_empty() || note.tags.is_empty() {
        return 0.0;
    }

    let matched = query_tags
        .iter()
        .filter(|qt| note.tags.iter().any(|nt| nt == *qt))
        .count();

    matched as f64 / query_tags.len() as f64
}

fn temporal_weight(updated_at: &DateTime<Utc>, now: DateTime<Utc>, decay_days: f64) -> f64 {
    let age = (now - *updated_at).num_hours() as f64 / (decay_days * 24.0);
    (-age).exp()
}
