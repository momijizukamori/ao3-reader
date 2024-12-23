use regex::Regex;
use scraper::Html;
use crate::html::{scrape, scrape_inner_text, scrape_inner_text_to_html, scrape_link_list, scrape_outer, Link};
use chrono::NaiveDate;
use serde::{Serialize, Deserialize};
use crate::helpers::date_format;
use crate::view::icon::DisabledIcon;
use crate::geom::Rectangle;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Rating{
    NotRated,
    Explicit,
    Mature,
    Teen,
    General
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Category{
    Slash,
    Femslash,
    Het,
    Gen,
    Multi,
    Other
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Warning{
    Yes,
    No,
    ChoseNotTo,
    External
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RequiredTags {
    pub rating: Rating,
    pub warnings: Warning,
    pub category: Category,
    pub complete: bool
}

impl Default for RequiredTags {
    fn default() -> Self {
        RequiredTags {
            rating: Rating::NotRated,
            warnings: Warning::ChoseNotTo,
            category: Category::Multi,
            complete: false
        }
    }
}

// TODO: add chapters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Ao3Info {
    pub id: String,
    pub title: String,
    pub authors: Vec<Link>,
    pub fandoms: Vec<Link>,
    pub req_tags: RequiredTags,
    pub tags: Vec<Link>,
    pub summary: String,
    pub kudos: usize,
    pub hits: usize,
    pub bookmarks: usize,
    pub comments: usize,
    pub words: usize,
    pub chapters: String,
    #[serde(with = "date_format")]
    pub updated: NaiveDate,
}

impl Default for Ao3Info {
    fn default() -> Self {
        Ao3Info {
            id: "0".to_string(),
            title: "Unknown Title".to_string(),
            authors: vec![],
            fandoms: vec![],
            req_tags: RequiredTags::default(),
            tags: vec![],
            summary: "No summary".to_string(),
            chapters: "0/0".to_string(),
            kudos: 0,
            hits: 0,
            comments: 0,
            words: 0,
            bookmarks: 0,
            updated: NaiveDate::MIN
        }
    }
}

pub fn str_to_usize(str: String) -> usize {
    str.replace(",", "").parse::<usize>().unwrap_or(0)
}

impl Ao3Info {
    pub fn new(data: String) -> Ao3Info {
        let html = Html::parse_fragment(&data);

        let mut id = "0".to_string(); 
        let id_re = Regex::new(r"work_(\d+)").unwrap();
        if let Some(caps) = id_re.captures(&data) {
            id = caps[1].to_string();
        }
        let datetime = scrape(&html, ".datetime");
        let updated = NaiveDate::parse_from_str(&datetime, "%d %b %Y").unwrap_or(NaiveDate::MIN);
        let title = scrape(&html, "h4.heading a");
        let authors = scrape_link_list(&html, r#"a[rel="author"]"#);
        let fandoms = scrape_link_list(&html, ".fandoms a");
        let req_tags = RequiredTags::new(scrape_outer(&html, "ul.required-tags"));
        let tags = scrape_link_list(&html, "ul.tags a");
        let summary = scrape_inner_text_to_html(&html, "blockquote.summary");
        let words = str_to_usize(scrape(&html, "dd.words"));
        let comments = str_to_usize(scrape(&html, "dd.comments"));
        let kudos = str_to_usize(scrape(&html, "dd.kudos"));
        let hits = str_to_usize(scrape(&html, "dd.hits"));
        let bookmarks = str_to_usize(scrape(&html, "dd.bookmarks a"));
        let chapters = scrape_inner_text(&html, "dd.chapters");

        Ao3Info{
            id,
            title,
            authors,
            fandoms,
            req_tags,
            tags,
            summary,
            words,
            comments,
            kudos,
            hits,
            bookmarks,
            updated,
            chapters
        }
    }

    pub fn new_from_work(meta: String, preface: String, id: String) -> Ao3Info {
        let html = Html::parse_fragment(&meta);
        let header = Html::parse_fragment(&preface);
        // TODO: req tags implementation

        let datetime = scrape(&html, ".status");
        let updated = NaiveDate::parse_from_str(&datetime, "%Y-%b-%d").unwrap_or(NaiveDate::MIN);
        let title = scrape(&header, "h2.title");
        let authors = scrape_link_list(&header, r#"a[rel="author"]"#);
        let fandoms = scrape_link_list(&html, ".fandom a");
        let req_tags = RequiredTags::new(scrape_outer(&html, "ul.required-tags"));

        let ships = scrape_link_list(&html, ".relationship a");
        let chars = scrape_link_list(&html, ".character a");
        let addl_tags = scrape_link_list(&html, ".freeform a");
        let mut tags = Vec::new();
        tags.extend(ships);
        tags.extend(chars);
        tags.extend(addl_tags);

        let summary = scrape_inner_text(&header, ".summary blockquote");
        let words = str_to_usize(scrape(&html, "dd.words"));
        let chapters = scrape_inner_text(&html, "dd.chapters");
        let comments = str_to_usize(scrape(&html, "dd.comments"));
        let kudos = str_to_usize(scrape(&html, "dd.kudos"));
        let hits = str_to_usize(scrape(&html, "dd.hits"));
        let bookmarks = str_to_usize(scrape(&html, "dd.bookmarks a"));

        Ao3Info{
            id,
            title,
            authors,
            fandoms,
            req_tags,
            tags,
            summary,
            words,
            comments,
            kudos,
            hits,
            bookmarks,
            updated,
            chapters
        }
    }


    pub fn download_name(&self) -> String {
        format!("{}.epub", self.title)
    }
}

impl RequiredTags {
    pub fn new(data: String) -> RequiredTags {
        let warnings_re = Regex::new(r"warning-(.+)").unwrap();
        let category_re = Regex::new(r"category-(.+)").unwrap();
        let complete_re = Regex::new(r"complete-(.+)").unwrap();
        let rating_re = Regex::new(r"rating-(.+)").unwrap();

        let mut warnings = Warning::ChoseNotTo;
        let mut category = Category::Multi;
        let mut complete = false;
        let mut rating = Rating::NotRated;

        if let Some(caps) = warnings_re.captures(&data) {
            warnings = match &caps[1] {
                "no" => Warning::No,
                "yes" => Warning::Yes,
                "choosenotto" => Warning::ChoseNotTo,
                "external" => Warning::External,
                &_ => Warning::ChoseNotTo,
            };
        }

        if let Some(caps) = category_re.captures(&data) {
            category = match &caps[1] {
                "multi" =>Category::Multi,
                "other" => Category::Other,
                "femslash" => Category::Femslash,
                "gen" => Category::Gen,
                "het" => Category::Het,
                &_ => Category::Multi,
            };
        }

        if let Some(caps) = complete_re.captures(&data) {
            complete = match &caps[1] {
                "no" => false,
                "yes" => true,
                &_ => false
            };
        }

        if let Some(caps) = rating_re.captures(&data) {
            rating = match &caps[1] {
                "explicit" => Rating::Explicit,
                "mature" => Rating::Mature,
                "teen" => Rating::Teen,
                "general-audience" => Rating::General,
                "notrated" => Rating::NotRated,
                &_ => Rating::NotRated,
            };
        }
        
        RequiredTags{
            rating,
            category,
            complete,
            warnings
        }
    }

    pub fn as_icons(&self, rect: Rectangle) -> Vec<DisabledIcon> {
        let width = rect.width();
        let height = rect.height();

        let mid_x = (width / 2) as i32;
        let mid_y = (height / 2) as i32;

        let rating_rect = rect![rect.min.x, rect.min.y, mid_x, mid_y];
        let cat_rect = rect![mid_x, rect.min.y, rect.max.x, mid_y];
        let warn_rect = rect![rect.min.x, mid_y, mid_x, rect.max.y];
        let complete_rect = rect![mid_x, mid_y, rect.max.x, rect.max.y];

        let rating_icon =  match self.rating {
            Rating::Explicit => DisabledIcon::new("explicit", rating_rect),
            Rating::Mature => DisabledIcon::new("mature", rating_rect),
            Rating::Teen => DisabledIcon::new("teen", rating_rect),
            Rating::General => DisabledIcon::new("general", rating_rect),
            Rating::NotRated => DisabledIcon::new("blank", rating_rect),
        };

        let category_icon = match self.category {
            Category::Slash => DisabledIcon::new("slash", cat_rect),
            Category::Femslash => DisabledIcon::new("femslash", cat_rect),
            Category::Het => DisabledIcon::new("het", cat_rect),
            Category::Multi => DisabledIcon::new("multi", cat_rect),
            Category::Other => DisabledIcon::new("other", cat_rect),
            Category::Gen => DisabledIcon::new("gen", cat_rect),
        };

        let complete_icon = if self.complete {DisabledIcon::new("complete", complete_rect)} else {DisabledIcon::new("wip", complete_rect)};

        let warning_icon = match self.warnings {
            Warning::Yes => DisabledIcon::new("warning", warn_rect),
            Warning::No => DisabledIcon::new("blank", warn_rect),
            Warning::ChoseNotTo => DisabledIcon::new("chosenotto", warn_rect),
            Warning::External => DisabledIcon::new("external", warn_rect),
        };

        vec![warning_icon, rating_icon, category_icon, complete_icon]
    }

}
