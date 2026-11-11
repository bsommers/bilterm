//! Reader-side cache slot attached to a [`super::VtParser`].
//!
//! The renderer (`crate::ui`, a higher layer) parks data it derived from the grid here, so the
//! data lives and dies with the parser instead of in a global map keyed by pane id. The slot is
//! type-erased because `vt` must not name `ui` types; it holds at most one value, and the reader
//! bounds what that value contains. Validity is the reader's job: it keys its data on
//! [`super::VtParser::generation`] plus every other input it used.

use std::any::Any;
use std::sync::Mutex;

/// One interior-mutable, type-erased value. Never read by the parser itself.
#[derive(Default)]
pub struct DerivedSlot(Mutex<Option<Box<dyn Any + Send + Sync>>>);

impl DerivedSlot {
    /// Runs `f` on the stored `T`, creating `T::default()` first when the slot is empty or holds
    /// another type. A poisoned lock (a reader panicked mid-update) drops the stored value.
    pub fn with<T, R>(&self, f: impl FnOnce(&mut T) -> R) -> R
    where
        T: Any + Send + Sync + Default,
    {
        let mut guard = match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = None;
                guard
            }
        };
        if let Some(value) = guard.as_mut().and_then(|boxed| boxed.downcast_mut::<T>()) {
            return f(value);
        }
        let mut fresh = T::default();
        let out = f(&mut fresh);
        *guard = Some(Box::new(fresh));
        out
    }

    /// Drops the stored value.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_keeps_one_value_and_replaces_it_on_type_change() {
        let slot = DerivedSlot::default();
        assert_eq!(slot.with(|v: &mut Vec<u8>| { v.push(1); v.len() }), 1);
        assert_eq!(slot.with(|v: &mut Vec<u8>| { v.push(2); v.len() }), 2);
        assert_eq!(slot.with(|s: &mut String| s.len()), 0);
        assert_eq!(slot.with(|v: &mut Vec<u8>| v.len()), 0, "the Vec was replaced by the String");
        slot.clear();
        assert_eq!(slot.with(|s: &mut String| s.len()), 0);
    }
}
