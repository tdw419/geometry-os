use super::types::*;
use super::Vm;

impl Vm {
    // --- Phase 50: Reactive Canvas Formula Engine ---

    /// Register a formula. Returns false if limits exceeded or cycle detected.
    pub fn formula_register(&mut self, target_idx: usize, deps: Vec<usize>, op: FormulaOp) -> bool {
        if self.formulas.len() >= MAX_FORMULAS {
            return false;
        }
        if deps.len() > MAX_FORMULA_DEPS {
            return false;
        }
        if target_idx >= CANVAS_RAM_SIZE {
            return false;
        }
        for &d in &deps {
            if d >= CANVAS_RAM_SIZE {
                return false;
            }
        }

        // Remove any existing formula targeting the same cell
        self.formula_remove(target_idx);

        // Cycle detection: adding this formula must not create a cycle.
        // A cycle exists if target_idx is reachable from any of its deps
        // through the existing formula graph.
        if self.has_formula_cycle(target_idx, &deps) {
            return false;
        }

        let fidx = self.formulas.len();
        let formula = Formula {
            target_idx,
            deps: deps.clone(),
            op,
        };
        self.formulas.push(formula);

        // Update reverse dependency index
        for &dep in &deps {
            if dep < self.formula_dep_index.len() {
                self.formula_dep_index[dep].push(fidx);
            }
        }
        true
    }

    /// Remove the formula targeting `target_idx`, if any.
    pub fn formula_remove(&mut self, target_idx: usize) {
        if let Some(pos) = self
            .formulas
            .iter()
            .position(|f| f.target_idx == target_idx)
        {
            // Remove from dep index
            for dep_list in self.formula_dep_index.iter_mut() {
                dep_list.retain(|&fi| fi != pos);
                // Shift indices > pos down by 1 since we're removing
                for fi in dep_list.iter_mut() {
                    if *fi > pos {
                        *fi -= 1;
                    }
                }
            }
            self.formulas.remove(pos);
        }
    }

    /// Check if adding a formula from deps -> target would create a cycle.
    pub(super) fn has_formula_cycle(&self, target_idx: usize, deps: &[usize]) -> bool {
        // A cycle exists if target_idx is transitively depended upon by any dep.
        // Walk the dependency graph from each dep and see if we reach target_idx.
        let mut visited = std::collections::HashSet::new();
        let mut stack: Vec<usize> = deps.to_vec();
        while let Some(idx) = stack.pop() {
            if idx == target_idx {
                return true;
            }
            if !visited.insert(idx) {
                continue;
            }
            // Find formulas that target this idx -- their deps could reach target
            for f in &self.formulas {
                if f.target_idx == idx {
                    stack.extend_from_slice(&f.deps);
                }
            }
        }
        false
    }

    /// Evaluate a single formula given current canvas buffer state.
    pub(super) fn formula_eval(&self, formula: &Formula, canvas: &[u32]) -> u32 {
        let get = |idx: usize| -> u32 {
            if idx < canvas.len() {
                canvas[idx]
            } else {
                0
            }
        };
        match formula.op {
            FormulaOp::Add => get(formula.deps[0]).wrapping_add(get(formula.deps[1])),
            FormulaOp::Sub => get(formula.deps[0]).wrapping_sub(get(formula.deps[1])),
            FormulaOp::Mul => get(formula.deps[0]).wrapping_mul(get(formula.deps[1])),
            FormulaOp::Div => {
                let d = get(formula.deps[1]);
                if d == 0 {
                    0
                } else {
                    get(formula.deps[0]) / d
                }
            }
            FormulaOp::And => get(formula.deps[0]) & get(formula.deps[1]),
            FormulaOp::Or => get(formula.deps[0]) | get(formula.deps[1]),
            FormulaOp::Xor => get(formula.deps[0]) ^ get(formula.deps[1]),
            FormulaOp::Not => !get(formula.deps[0]),
            FormulaOp::Copy => get(formula.deps[0]),
            FormulaOp::Max => get(formula.deps[0]).max(get(formula.deps[1])),
            FormulaOp::Min => get(formula.deps[0]).min(get(formula.deps[1])),
            FormulaOp::Mod => {
                let d = get(formula.deps[1]);
                if d == 0 {
                    0
                } else {
                    get(formula.deps[0]) % d
                }
            }
            FormulaOp::Shl => get(formula.deps[0]).wrapping_shl(get(formula.deps[1]) % 32),
            FormulaOp::Shr => get(formula.deps[0]).wrapping_shr(get(formula.deps[1]) % 32),
        }
    }

    /// Recalculate all formulas that depend on `changed_idx`.
    /// Called after a STORE to a canvas cell.
    pub fn formula_recalc(&mut self, changed_idx: usize) {
        if changed_idx >= self.formula_dep_index.len() {
            return;
        }
        let affected: Vec<usize> = self.formula_dep_index[changed_idx].clone();
        if affected.is_empty() {
            return;
        }

        // Evaluate all affected formulas (use a snapshot to avoid borrow issues)
        let canvas_snapshot = self.canvas_buffer.clone();
        let mut updates: Vec<(usize, u32)> = Vec::new();
        for &fidx in &affected {
            if fidx < self.formulas.len() {
                let result = self.formula_eval(&self.formulas[fidx], &canvas_snapshot);
                updates.push((self.formulas[fidx].target_idx, result));
            }
        }
        // Apply updates
        for (idx, val) in updates {
            if idx < self.canvas_buffer.len() {
                self.canvas_buffer[idx] = val;
            }
        }
    }

    /// Clear all formulas and rebuild the dependency index.
    pub fn formula_clear_all(&mut self) {
        self.formulas.clear();
        for dep_list in self.formula_dep_index.iter_mut() {
            dep_list.clear();
        }
    }
}
