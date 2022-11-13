use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use lsp_types::CallHierarchyItem;

#[derive(Clone)]
pub struct HashableCallHierarchyItem(pub CallHierarchyItem);

impl std::fmt::Debug for HashableCallHierarchyItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("HashableCallHierarchyItem({})", self.0.name))
    }
}

impl Hash for HashableCallHierarchyItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.range.start.line.hash(state);
        self.0.range.start.character.hash(state);
        self.0.range.end.line.hash(state);
        self.0.range.end.character.hash(state);
        self.0.uri.hash(state);
    }
}

impl PartialEq for HashableCallHierarchyItem {
    fn eq(&self, other: &Self) -> bool {
        let mut s1 = DefaultHasher::new();
        self.hash(&mut s1);
        let h1 = s1.finish();

        let mut s2 = DefaultHasher::new();
        other.hash(&mut s2);
        let h2 = s2.finish();

        h1 == h2
    }
}

impl Eq for HashableCallHierarchyItem {}

impl From<CallHierarchyItem> for HashableCallHierarchyItem {
    fn from(call_hierarchy_item: CallHierarchyItem) -> Self {
        Self(call_hierarchy_item)
    }
}

impl From<HashableCallHierarchyItem> for CallHierarchyItem {
    fn from(hashable_call_hierarchy_item: HashableCallHierarchyItem) -> Self {
        hashable_call_hierarchy_item.0
    }
}
