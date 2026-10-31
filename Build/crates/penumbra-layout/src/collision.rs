use std::collections::HashSet;

use penumbra_core::note::NoteId;
use penumbra_core::position::{Bounds, Position};

use crate::cpu::SpatialHash;
use crate::LayoutConfig;

/// Read-only view of everything a collision pass needs from the engine.
pub(crate) struct CollisionField<'a> {
    pub(crate) nodes: &'a [NoteId],
    pub(crate) node_bounds: &'a std::collections::HashMap<NoteId, Bounds>,
    pub(crate) pinned: &'a HashSet<NoteId>,
    pub(crate) config: &'a LayoutConfig,
}

impl CollisionField<'_> {
    /// The widest distance at which two boxes can still overlap, used to size
    /// the spatial-hash query so no overlapping pair is missed
    fn search_radius(&self) -> f64 {
        let mut max_half_diag = 0.0f64;
        for bounds in self.node_bounds.values() {
            max_half_diag = max_half_diag.max(half_diag(bounds));
        }
        max_half_diag * 2.0 + self.config.collision_margin
    }

    fn is_pinned(&self, index: usize) -> bool {
        self.pinned.contains(&self.nodes[index])
    }

    fn bounds_of(&self, index: usize) -> Option<&Bounds> {
        self.node_bounds.get(&self.nodes[index])
    }
}

/// The separation to apply to one overlapping pair `(i, j)`, or `None` when the
/// pair's bounding boxes do not actually overlap.
fn pair_push(
    field: &CollisionField<'_>,
    positions: &[Position],
    i: usize,
    j: usize,
) -> Option<(f64, f64, f64)> {
    let (bounds_i, bounds_j) = (field.bounds_of(i)?, field.bounds_of(j)?);
    let (a, b) = (positions[i], positions[j]);
    if !bounds_i.overlaps(&a, bounds_j, &b) {
        return None;
    }

    let ca_x = a.x + bounds_i.width * 0.5;
    let ca_y = a.y + bounds_i.height * 0.5;
    let cb_x = b.x + bounds_j.width * 0.5;
    let cb_y = b.y + bounds_j.height * 0.5;

    let dx = cb_x - ca_x;
    let dy = cb_y - ca_y;

    // Required center spacing along each axis for the boxes not to overlap,
    // including the gap margin.
    let margin = field.config.collision_margin;
    let sep_x = (bounds_i.width + bounds_j.width) * 0.5 + margin;
    let sep_y = (bounds_i.height + bounds_j.height) * 0.5 + margin;

    // Penetration depth on each axis (positive means overlapping on that axis).
    let pen_x = sep_x - dx.abs();
    let pen_y = sep_y - dy.abs();
    if pen_x <= 0.0 || pen_y <= 0.0 {
        // A positive gap on either axis means the boxes already clear.
        return None;
    }

    // Resolve along the axis of least penetration (the minimum translation).
    let (nx, ny, penetration) = if pen_x < pen_y {
        let sign = if dx.abs() < 1e-6 {
            // Coincident on x: deterministic direction from the node indices.
            if (i.wrapping_mul(2654435761) ^ j.wrapping_mul(2246822519)) & 1 == 0 {
                1.0
            } else {
                -1.0
            }
        } else {
            dx.signum()
        };
        (sign, 0.0, pen_x)
    } else {
        let sign = if dy.abs() < 1e-6 {
            if (i.wrapping_mul(2654435761) ^ j.wrapping_mul(2246822519)) & 2 == 0 {
                1.0
            } else {
                -1.0
            }
        } else {
            dy.signum()
        };
        (0.0, sign, pen_y)
    };

    let push = penetration.min(field.config.max_collision_push);
    Some((nx, ny, push))
}

/// Apply a resolved pair push to `pos`, distributing the separation by pin
/// state
fn apply_pair_push(
    field: &CollisionField<'_>,
    pos: &mut [Position],
    i: usize,
    j: usize,
    nx: f64,
    ny: f64,
    push: f64,
) {
    let i_pinned = field.is_pinned(i);
    let j_pinned = field.is_pinned(j);
    match (i_pinned, j_pinned) {
        // Both pinned: neither may move; leave the overlap to the pins.
        (true, true) => {}
        // Only j is pinned: i takes the whole separation, moving away from j.
        (false, true) => {
            pos[i].x -= nx * push;
            pos[i].y -= ny * push;
        }
        // Only i is pinned: j takes the whole separation, moving away from i.
        (true, false) => {
            pos[j].x += nx * push;
            pos[j].y += ny * push;
        }
        // Neither pinned: split the separation evenly between them.
        (false, false) => {
            let half = push * 0.5;
            pos[i].x -= nx * half;
            pos[i].y -= ny * half;
            pos[j].x += nx * half;
            pos[j].y += ny * half;
        }
    }
}

/// Half the diagonal of a bounding box; the radius of the circle that bounds it.
fn half_diag(bounds: &Bounds) -> f64 {
    (bounds.width * bounds.width + bounds.height * bounds.height).sqrt() * 0.5
}

/// Push apart every overlapping node pair across the whole graph. Returns the
/// resolved positions aligned with `positions`. Pinned nodes are never moved.
pub(crate) fn resolve_all(
    field: &CollisionField<'_>,
    positions: &[Position],
) -> Vec<Position> {
    let search_radius = field.search_radius();
    let mut pos = positions.to_vec();

    for _pass in 0..field.config.collision_passes {
        let mut any_resolved = false;
        // Rebuild the index each pass so recently pushed nodes are found in
        // their new cells on the next iteration.
        let hash = SpatialHash::new(search_radius, pos.clone());

        for i in 0..field.nodes.len() {
            if field.is_pinned(i) {
                continue;
            }
            let Some(bounds_i) = field.bounds_of(i) else {
                continue;
            };
            let center_a = Position::new(
                pos[i].x + bounds_i.width * 0.5,
                pos[i].y + bounds_i.height * 0.5,
            );

            for j in hash.within(center_a, search_radius) {
                if j == i {
                    continue;
                }

                if !field.is_pinned(j) && j < i {
                    continue;
                }
                let Some((nx, ny, push)) = pair_push(field, &pos, i, j) else {
                    continue;
                };
                apply_pair_push(field, &mut pos, i, j, nx, ny, push);
                any_resolved = true;
            }
        }

        if !any_resolved {
            break;
        }
    }

    pos
}

/// Push apart overlapping node pairs within `radius` of `center`, leaving the
/// rest of the graph fixed.
pub(crate) fn resolve_neighborhood(
    field: &CollisionField<'_>,
    positions: &[Position],
    center: usize,
    radius: f64,
) -> Vec<Position> {
    let search_radius = field.search_radius();
    let mut pos = positions.to_vec();
    let center_pos = pos[center];

    let initial_hash = SpatialHash::new(search_radius, pos.clone());
    let neighborhood: HashSet<usize> = initial_hash
        .within(center_pos, radius + search_radius)
        .into_iter()
        .collect();

    for _pass in 0..field.config.collision_passes {
        let mut any_resolved = false;
        let hash = SpatialHash::new(search_radius, pos.clone());
        let mut neighbors: Vec<usize> = hash.within(center_pos, radius + search_radius);
        neighbors.sort_unstable();
        neighbors.dedup();

        for &i in &neighbors {
            if !neighborhood.contains(&i) || field.is_pinned(i) {
                continue;
            }
            let Some(bounds_i) = field.bounds_of(i) else {
                continue;
            };
            let center_a = Position::new(
                pos[i].x + bounds_i.width * 0.5,
                pos[i].y + bounds_i.height * 0.5,
            );

            for j in hash.within(center_a, search_radius) {
                if j == i {
                    continue;
                }
                // Same pin-aware pair selection as `resolve_all`: process a
                // movable pair once, but always evaluate a pinned `j`.
                if !field.is_pinned(j) && j < i {
                    continue;
                }
                let Some((nx, ny, push)) = pair_push(field, &pos, i, j) else {
                    continue;
                };
                if neighborhood.contains(&j) {
                    // Both are solver-visible: honour pins via the shared split.
                    apply_pair_push(field, &mut pos, i, j, nx, ny, push);
                } else {
                    // j is an anchor outside the neighborhood (fixed like a
                    // pin); only i moves, and it absorbs the full separation.
                    pos[i].x -= nx * push;
                    pos[i].y -= ny * push;
                }
                any_resolved = true;
            }
        }

        if !any_resolved {
            break;
        }
    }

    // Only neighborhood members are written back; anchors keep their input.
    for i in 0..pos.len() {
        if !neighborhood.contains(&i) {
            pos[i] = positions[i];
        }
    }
    pos
}
