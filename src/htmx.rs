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
            created_at,
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
                date: created_at.to_string(),
                tags: tags.map(|tags| DocumentTags {
                    tags: tags
                        .split(',')
                        .filter_map(|e| (!e.is_empty()).then_some(e.to_string()))
                        .collect(),
                }),
            }),
            content,
        }
    }
}

#[derive(Debug, Default, htmxpress::Element)]
#[element("li")]
#[attrs(class = "sidebar-container")]
pub struct SidebarContainer {
    #[element("h2")]
    pub root_name: String,

    #[list(nest)]
    pub documents: Vec<SidebarDocumentHtmx>,

    #[list(nest)]
    pub directories: Vec<SidebarDirectoryHtmx>,
}

impl SidebarContainer {
    pub fn new(name: String) -> Self {
        Self {
            root_name: name,
            ..Default::default()
        }
    }
}

#[derive(Debug, htmxpress::Element)]
#[element("div")]
#[attrs(class = "sidebar-document")]
#[hx_get("/main/{}", path)]
#[hx("target" = "#main")]
#[attr("hx-push-url" = "/{}", path)]
pub struct SidebarDocumentHtmx {
    #[element("h2")]
    pub title: String,

    pub path: uuid::Uuid,
}

impl SidebarDocumentHtmx {
    pub fn new(title: String, path: uuid::Uuid) -> Self {
        Self { title, path }
    }
}

#[derive(Debug, htmxpress::Element)]
#[element("div")]
#[attrs(class = "sidebar-directory")]
#[hx_get("/side/{}", path)]
#[hx("trigger" = "click once")]
#[attr("hx-target" = "#_{}", path)]
pub struct SidebarDirectoryHtmx {
    #[element("h2")]
    pub title: String,

    pub path: uuid::Uuid,

    #[element("div")]
    #[attr("id" = "_{}", path)]
    pub sub: String,
}

impl SidebarDirectoryHtmx {
    pub fn new(title: String, path: uuid::Uuid) -> Self {
        Self {
            title,
            path,
            sub: String::new(),
        }
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
