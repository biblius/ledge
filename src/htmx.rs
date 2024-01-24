use crate::document::Document;

#[derive(Debug, htmxpress::Element)]
pub struct MainDocumentHtmx {
    #[nest]
    pub meta: Option<DocumentHeaderInfo>,

    #[element("div")]
    #[attrs(class = "document-content")]
    pub content: String,
}

impl From<Document> for MainDocumentHtmx {
    fn from(value: Document) -> Self {
        let Document {
            content,
            created,
            reading_time,
            tags,
            ..
        }: Document = value;
        Self {
            // Last property to get mapped, so if this is not here
            // we know there's not meta for the document
            meta: reading_time.map(|reading_time| DocumentHeaderInfo {
                reading_time,
                tags_p: "Tags:",
                date: created.to_string(),
                tags: (!tags.is_empty()).then_some(DocumentTags { tags }),
            }),
            content,
        }
    }
}

#[derive(Debug, htmxpress::Element)]
#[element("li")]
#[attrs(class = "sidebar-document")]
#[hx_get("/main/{}", path)]
#[hx("target" = "#main")]
#[attr("hx-push-url" = "/{}", path)]
pub struct SidebarDocumentHtmx {
    #[element("h2")]
    pub title: String,

    pub path: String,
}

impl SidebarDocumentHtmx {
    pub fn new(title: String, path: String) -> Self {
        Self { title, path }
    }
}

#[derive(Debug, htmxpress::Element)]
#[element("div")]
#[attrs(class = "document-meta")]
pub struct DocumentHeaderInfo {
    #[element("p")]
    #[format("Reading time: {}m")]
    reading_time: i32,

    #[element("p")]
    #[format("Created: {}")]
    date: String,

    #[element("p")]
    tags_p: &'static str,

    #[nest]
    tags: Option<DocumentTags>,
}

#[derive(Debug, Default, htmxpress::Element)]
#[element("ul")]
#[attrs(class = "tag-container")]
pub struct DocumentTags {
    #[list]
    #[element("li")]
    #[format("#{}")]
    #[attrs(class = "tag")]
    tags: Vec<String>,
}
