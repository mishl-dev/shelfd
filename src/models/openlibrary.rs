use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OlResponse {
    pub docs: Vec<OlDoc>,
}

#[derive(Debug, Deserialize)]
pub struct OlDoc {
    pub cover_i: Option<i64>,
    pub cover_edition_key: Option<String>,
    pub subject: Option<Vec<String>>,
    pub first_publish_year: Option<i64>,
    pub language: Option<Vec<String>>,
    pub editions: Option<OlEditions>,
}

#[derive(Debug, Deserialize)]
pub struct OlEditions {
    pub docs: Vec<OlEditionDoc>,
}

#[derive(Debug, Deserialize)]
pub struct OlEditionDoc {
    pub key: Option<String>,
    pub cover_i: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OlSubjectResponse {
    pub works: Vec<OlSubjectWork>,
}

#[derive(Debug, Deserialize)]
pub struct OlSubjectWork {
    pub key: String,
    pub title: String,
    pub authors: Vec<OlAuthorRef>,
    pub cover_id: Option<i64>,
    pub edition_count: Option<i64>,
    pub subject: Option<Vec<String>>,
    pub first_publish_year: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OlAuthorRef {
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct OlEnrichment {
    pub cover_url: Option<String>,
    pub first_publish_year: Option<i64>,
    pub language: Option<String>,
    pub subjects: Vec<String>,
}
