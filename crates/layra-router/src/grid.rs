//! Uniform spatial grid for rect collision candidate queries.
//!
//! Simpler and faster to build than an R-tree for our access pattern:
//! built once per route() call, queried once or twice per edge. Cell size
//! adapts to the average node footprint.

use layra_core::Rect;

pub(crate) struct SpatialGrid {
    cell: f32,
    cols: usize,
    rows: usize,
    origin: (f32, f32),
    /// Node indices per cell.
    cells: Vec<Vec<u32>>,
    /// Generation marks for dedup during query (avoids a HashSet).
    seen: std::cell::RefCell<(Vec<u32>, u32)>,
    len: usize,
}

impl SpatialGrid {
    pub fn build(rects: &[Rect]) -> Self {
        if rects.is_empty() {
            return Self {
                cell: 1.0,
                cols: 1,
                rows: 1,
                origin: (0.0, 0.0),
                cells: vec![Vec::new()],
                seen: std::cell::RefCell::new((Vec::new(), 0)),
                len: 0,
            };
        }

        let bounds = rects[1..]
            .iter()
            .fold(rects[0], |acc, r| acc.union(r))
            .inflate(1.0);

        // Cell ≈ 2x the average node diagonal: most rects land in 1-4 cells.
        let avg = rects
            .iter()
            .map(|r| (r.width + r.height) / 2.0)
            .sum::<f32>()
            / rects.len() as f32;
        let cell = (avg * 2.0).max(16.0);

        let cols = ((bounds.width / cell).ceil() as usize).max(1);
        let rows = ((bounds.height / cell).ceil() as usize).max(1);
        let mut cells = vec![Vec::new(); cols * rows];

        let origin = (bounds.x, bounds.y);
        for (i, r) in rects.iter().enumerate() {
            let c0 = (((r.x - origin.0) / cell) as usize).min(cols - 1);
            let c1 = (((r.right() - origin.0) / cell) as usize).min(cols - 1);
            let r0 = (((r.y - origin.1) / cell) as usize).min(rows - 1);
            let r1 = (((r.bottom() - origin.1) / cell) as usize).min(rows - 1);
            for row in r0..=r1 {
                for col in c0..=c1 {
                    cells[row * cols + col].push(i as u32);
                }
            }
        }

        Self {
            cell,
            cols,
            rows,
            origin,
            cells,
            seen: std::cell::RefCell::new((vec![0; rects.len()], 0)),
            len: rects.len(),
        }
    }

    /// Collect indices of rects whose cells intersect `region` into `out`
    /// (cleared first, deduplicated).
    pub fn query(&self, region: &Rect, out: &mut Vec<usize>) {
        out.clear();
        if self.len == 0 {
            return;
        }
        let mut seen = self.seen.borrow_mut();
        seen.1 = seen.1.wrapping_add(1);
        let generation = seen.1;

        let c0 = (((region.x - self.origin.0) / self.cell).max(0.0) as usize).min(self.cols - 1);
        let c1 =
            (((region.right() - self.origin.0) / self.cell).max(0.0) as usize).min(self.cols - 1);
        let r0 = (((region.y - self.origin.1) / self.cell).max(0.0) as usize).min(self.rows - 1);
        let r1 =
            (((region.bottom() - self.origin.1) / self.cell).max(0.0) as usize).min(self.rows - 1);

        for row in r0..=r1 {
            for col in c0..=c1 {
                for &i in &self.cells[row * self.cols + col] {
                    if seen.0[i as usize] != generation {
                        seen.0[i as usize] = generation;
                        out.push(i as usize);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_local_rects() {
        let rects: Vec<Rect> = (0..100)
            .map(|i| Rect::new((i % 10) as f32 * 100.0, (i / 10) as f32 * 100.0, 50.0, 30.0))
            .collect();
        let grid = SpatialGrid::build(&rects);

        let mut out = Vec::new();
        grid.query(&Rect::new(0.0, 0.0, 120.0, 120.0), &mut out);
        assert!(out.contains(&0));
        assert!(!out.contains(&99), "far rect must be pruned");
        assert!(out.len() < 20, "query should be local, got {}", out.len());
    }

    #[test]
    fn empty_grid_is_safe() {
        let grid = SpatialGrid::build(&[]);
        let mut out = vec![123];
        grid.query(&Rect::new(0.0, 0.0, 10.0, 10.0), &mut out);
        assert!(out.is_empty());
    }
}
