use bdat::Label;

pub struct Filter {
    hashes: Vec<u32>,
}

pub struct FilterArg(pub String);

impl Filter {
    pub fn contains(&self, label: &Label) -> bool {
        if self.hashes.is_empty() {
            return true;
        }

        let hash = match label {
            Label::Hash(h) => *h,
            Label::String(s) => Self::hash(s),
        };
        self.hashes.binary_search(&hash).is_ok()
    }

    fn hash(key: &str) -> u32 {
        bdat::hash::murmur3_str(key)
    }
}

impl FromIterator<FilterArg> for Filter {
    fn from_iter<T: IntoIterator<Item = FilterArg>>(iter: T) -> Self {
        Self::from_iter(iter.into_iter().flat_map(|s| {
            match u32::from_str_radix(&s.0, 16) {
                Ok(n) => [Some(Label::Hash(n)), Some(s.0.into())]
                    .into_iter()
                    .flatten(),
                Err(_) => [Some(s.0.into()), None].into_iter().flatten(),
            }
        }))
    }
}

impl<'b> FromIterator<Label<'b>> for Filter {
    fn from_iter<T: IntoIterator<Item = Label<'b>>>(iter: T) -> Self {
        let mut hashes = iter
            .into_iter()
            .map(|l| match l {
                Label::Hash(h) => h,
                Label::String(s) => Self::hash(&s),
            })
            .collect::<Vec<_>>();
        hashes.sort_unstable();
        Self { hashes }
    }
}
