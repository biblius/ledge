use crate::document::{DocumentData, DocumentMeta};

#[derive(Debug, htmxpress::Element)]
pub struct MainDocumentHtmx {
    #[nest]
    pub head: Option<DocumentHeadHtmx>,

    #[nest]
    pub meta: Option<DocumentHeaderInfo>,

    #[element("div")]
    #[attrs(class = "document-content")]
    pub content: String,
}

impl MainDocumentHtmx {
    pub fn new_page(data: DocumentData) -> Self {
        let DocumentData {
            content,
            meta: DocumentMeta {
                reading_time, tags, ..
            },
        } = data;
        Self {
            head: None,
            meta: reading_time.map(|reading_time| DocumentHeaderInfo {
                reading_time,
                tags_p: "Tags:",
                tags: tags.map(|tags| (DocumentTags { tags })),
                date: None, //TODO
            }),
            content,
        }
    }

    pub fn new_main(head: Option<DocumentHeadHtmx>, data: DocumentData) -> Self {
        let DocumentData {
            content,
            meta: DocumentMeta {
                reading_time, tags, ..
            },
        } = data;
        Self {
            head,
            meta: reading_time.map(|reading_time| DocumentHeaderInfo {
                reading_time,
                tags_p: "Tags:",
                date: None, // TODO
                // date: created_at.map(|c| c.to_string()),
                tags: tags.map(|tags| (DocumentTags { tags })),
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

    pub id: uuid::Uuid,
}

impl SidebarContainer {
    pub fn new(id: uuid::Uuid, name: String) -> Self {
        Self {
            id,
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

    pub path: String,
}

impl SidebarDocumentHtmx {
    pub fn new(title: String, path: String) -> Self {
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

    #[element("div")]
    #[attr("id" = "_{}", path)]
    pub sub: String,

    pub path: String,
}

impl SidebarDirectoryHtmx {
    pub fn new(title: String, path: String) -> Self {
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
    date: Option<String>,

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

#[derive(Debug, Default, htmxpress::Element)]
#[element("head")]
pub struct DocumentHeadHtmx {
    #[element("title")]
    title: String,
}

impl DocumentHeadHtmx {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
        }
    }
}
