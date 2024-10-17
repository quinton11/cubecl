use std::collections::{HashMap, HashSet};

use cubecl_core::ir::{CopyOperator, Operation, Operator, Variable};

use crate::{AtomicCounter, Optimizer};

use super::{item_compatible, OptimizerPass};

/// Transform consecutive `Index` and `IndexAssign` operations that don't modify the value into a
/// single `Copy` operation.
pub struct CopyTransform;

impl OptimizerPass for CopyTransform {
    fn apply_post_ssa(&mut self, opt: &mut Optimizer, changes: AtomicCounter) {
        for block in opt.node_ids() {
            let mut reads = HashMap::new();
            let mut writes = HashMap::new();
            let ops = opt.program[block].ops.clone();
            for (idx, op) in ops.borrow().iter() {
                match op {
                    Operation::Operator(Operator::Index(op))
                        if op.lhs.is_array() && item_compatible(op.lhs.item(), op.out.item()) =>
                    {
                        if let Some(id) = as_versioned(&op.out) {
                            reads.insert(id, (idx, op.lhs, op.rhs));
                        }
                    }
                    Operation::Operator(Operator::IndexAssign(op))
                        if op.out.is_array() && item_compatible(op.out.item(), op.rhs.item()) =>
                    {
                        if let Some(id) = as_versioned(&op.rhs) {
                            writes.insert(id, (idx, op.out, op.lhs));
                        }
                    }
                    _ => {}
                }
            }
            let read_ids: HashSet<_> = reads.keys().collect();
            let write_ids: HashSet<_> = writes.keys().collect();
            let copy_ids = read_ids.intersection(&write_ids);
            for id in copy_ids {
                let (read_idx, input, in_index) = reads[*id];
                let (write_idx, out, out_index) = writes[*id];
                ops.borrow_mut().remove(read_idx);
                let copy = Operator::Copy(CopyOperator {
                    out,
                    out_index,
                    input,
                    in_index,
                });
                ops.borrow_mut()[write_idx] = copy.into();
                changes.inc();
            }
        }
    }
}

fn as_versioned(var: &Variable) -> Option<(u16, u8, u16)> {
    match var {
        Variable::LocalBinding { id, depth, .. } => Some((*id, *depth, 0)),
        Variable::Versioned {
            id, depth, version, ..
        } => Some((*id, *depth, *version)),
        _ => None,
    }
}
