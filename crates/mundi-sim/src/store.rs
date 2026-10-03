//! Entities live in slots; a key is a slot and the generation it was filled in. A key kept after its
//! entity is gone finds nothing, even when the slot holds someone new.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key {
    index: u32,
    generation: u32,
}

#[derive(Debug)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

#[derive(Debug)]
pub struct Store<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Store { slots: Vec::new(), free: Vec::new() }
    }
}

impl<T> Store<T> {
    /// Free slots are reused lowest first, so the same inputs give the same keys.
    pub fn insert(&mut self, value: T) -> Key {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation += 1;
            slot.value = Some(value);
            Key { index, generation: slot.generation }
        } else {
            self.slots.push(Slot { generation: 0, value: Some(value) });
            Key { index: self.slots.len() as u32 - 1, generation: 0 }
        }
    }

    pub fn remove(&mut self, key: Key) -> Option<T> {
        let slot = self.slots.get_mut(key.index as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        let value = slot.value.take()?;
        self.free.push(key.index);
        self.free.sort_unstable_by(|a, b| b.cmp(a));
        Some(value)
    }

    pub fn get(&self, key: Key) -> Option<&T> {
        let slot = self.slots.get(key.index as usize)?;
        if slot.generation == key.generation { slot.value.as_ref() } else { None }
    }

    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        let slot = self.slots.get_mut(key.index as usize)?;
        if slot.generation == key.generation { slot.value.as_mut() } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_keys_find_nothing() {
        let mut s = Store::default();
        let a = s.insert("a");
        assert_eq!(s.remove(a), Some("a"));
        let b = s.insert("b");
        assert_eq!(s.get(a), None);
        assert_eq!(s.get(b), Some(&"b"));
        assert_eq!(s.remove(a), None);
    }
}
