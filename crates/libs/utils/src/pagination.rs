#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort<K> {
    pub key: K,
    pub order: SortOrder,
}

#[derive(Clone, Debug)]
pub struct Cursor<C> {
    pub after: Option<C>,
    pub limit: Option<u64>,
}

impl<C> Default for Cursor<C> {
    fn default() -> Self {
        Self {
            after: None,
            limit: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Page<T, C> {
    pub items: Vec<T>,
    pub next: Option<C>,
}

impl<T, C> Default for Page<T, C> {
    fn default() -> Self {
        Self {
            items: Vec::with_capacity(0),
            next: None,
        }
    }
}
