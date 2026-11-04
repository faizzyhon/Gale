use serde::Serialize;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub extensions: String,
    pub subfolder: String,
    pub is_builtin: bool,
    pub sort_order: i64,
}

/// Pick the category whose extension list contains the file's extension; falls back to "Other".
pub fn categorize<'a>(categories: &'a [Category], file_name: &str) -> Option<&'a Category> {
    let ext = file_name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    ext.as_deref()
        .and_then(|ext| categories.iter().find(|c| c.extensions.split_whitespace().any(|e| e == ext)))
        .or_else(|| categories.iter().find(|c| c.name == "Other"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(id: i64, name: &str, ext: &str) -> Category {
        Category { id, name: name.into(), extensions: ext.into(), subfolder: name.into(), is_builtin: true, sort_order: id }
    }

    #[test]
    fn maps_extensions_case_insensitively() {
        let cats = [cat(1, "Video", "mp4 mkv"), cat(2, "Other", "")];
        assert_eq!(categorize(&cats, "Movie.MKV").unwrap().name, "Video");
        assert_eq!(categorize(&cats, "notes").unwrap().name, "Other");
        assert_eq!(categorize(&cats, "x.unknown").unwrap().name, "Other");
    }
}
