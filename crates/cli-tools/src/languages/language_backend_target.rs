#[derive(Clone)]
pub struct LanguageBackendTarget {
    pub name: String,
    pub features: Vec<String>,
}

impl LanguageBackendTarget {
    pub fn new(
        name: String,
        features: Vec<String>,
    ) -> Self {
        Self {
            name,
            features,
        }
    }
}
