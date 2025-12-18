//! BitFont atomic undo guard

use crate::bitfont::BitFontOperationType;

/// Guard for grouping multiple operations into a single undo step
pub struct BitFontAtomicUndoGuard {
    pub(super) base_count: usize,
    pub(super) description: String,
    pub(super) operation_type: BitFontOperationType,
    pub(super) ended: bool,
}

impl BitFontAtomicUndoGuard {
    pub fn new(description: String, base_count: usize, operation_type: BitFontOperationType) -> Self {
        Self {
            base_count,
            description,
            operation_type,
            ended: false,
        }
    }

    /// End the atomic undo group explicitly
    /// Returns the base count and description for the caller to finalize
    pub fn end_params(&mut self) -> Option<(usize, String, BitFontOperationType)> {
        if self.ended {
            return None;
        }
        self.ended = true;
        Some((self.base_count, self.description.clone(), self.operation_type))
    }

    /// Get the base count (for use in BitFontEditState)
    pub fn base_count(&self) -> usize {
        self.base_count
    }

    /// Get the description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the operation type
    pub fn operation_type(&self) -> BitFontOperationType {
        self.operation_type
    }

    /// Check if already ended
    pub fn is_ended(&self) -> bool {
        self.ended
    }

    /// Mark as ended (called when finalized externally)
    pub fn mark_ended(&mut self) {
        self.ended = true;
    }
}

// Note: Drop implementation removed - the guard must be explicitly ended
// by passing it to BitFontEditState::end_atomic_undo_guard()
