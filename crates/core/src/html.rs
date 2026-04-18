use crate::helpers::decode_entities;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub title: String,
    pub location: String,
}

pub enum Format {
    Bold,
    Italic
}

pub enum HtmlText {
    Break,
    Blockquote,
    Text(String),
    LinkText(Link),
    FormatText(String, Format)
}

pub fn list_to_str(list: &Vec<Link>, sep: &str) -> String {
    let mut temp = Vec::new();
    for link in list {
        temp.push(link.title.clone());
    }
    temp.join(sep)
}



pub fn scrape_inner_text(frag: &Html, select: &str) -> String {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => {
            let raw_text = el.text().collect::<Vec<_>>().join("");
            let trimmed = raw_text.trim();
            let text = decode_entities(trimmed).into_owned();
            return text;
        }
        None => {
            return "####".to_string();
        }
    };
}

pub fn scrape_inner_text_to_html(frag: &Html, select: &str) -> String {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => {
            let mut html_els = Vec::new();
            for item in el.children() {
                 match item.value().clone() {
                    scraper::Node::Element(el) => {
                        println!("element is {:?}", el.name());
                        for item2 in item.children() {
                            println!("nested value is {:#?}", item2.value());
                        }
                    },
                    scraper::Node::Text(txt) => {
                        let text = txt.to_string();
                        if text == "\n" {
                            html_els.push(HtmlText::Break)
                        } else {
                            html_els.push(HtmlText::Text(text))
                        }
                    },
                    _ => {}

                 }
            }
            let raw_text = el.text().collect::<Vec<_>>().join("");
            let trimmed = raw_text.trim();
            let text = decode_entities(trimmed).into_owned();
            return text;
        }
        None => {
            return "####".to_string();
        }
    };
}

pub fn scrape_login_csrf(frag: &Html) -> String {
    let token = Selector::parse(r#"form.new_user input[name="authenticity_token"]"#).unwrap();
    println!("HTML inner: {}", frag.root_element().inner_html()); // DO NOT REMOVE - causes thread panic
    let input = frag.select(&token).next().unwrap();
    input.value().attr("value").unwrap().to_string()
}

pub fn scrape_kudos_csrf(frag: &Html) -> Option<&str> {
    let token = Selector::parse(r#"form#new_kudo input[name="authenticity_token"]"#).unwrap();
    let input = frag.select(&token).next();
    if let Some(input) = input {
        input.value().attr("value")
    } else {
        None
    }    
}

pub fn scrape(frag: &Html, select: &str) -> String {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => {
            let raw = el.inner_html();
            let trimmed = raw.trim();
            let clean = decode_entities(trimmed).into_owned();
            return clean;
        }
        None => {
            println!("error trying to scrape {}", select);
            return "#####".to_string();
        }
    };
}

pub fn scrape_link_list(frag: &Html, select: &str) -> Vec<Link> {
    let selector = Selector::parse(select).unwrap();
    let elems = frag.select(&selector);

    let mut results = Vec::new();
    for el in elems {
        let raw_title = el.inner_html();
        let trimmed = raw_title.trim();
        let title = decode_entities(trimmed).into_owned();
        let location = el.value().attr("href").unwrap_or("").to_string();
        results.push(Link { title, location });
    }

    results
}

pub fn scrape_link(frag: &Html, select: &str) -> Link {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => {
            let raw_title = el.inner_html();
            let trimmed = raw_title.trim();
            let title = decode_entities(trimmed).into_owned();
            let location = el.value().attr("href").unwrap_or("").to_string();
            return Link { title, location };
        }
        None => {
            return Link {
                title: "####".to_string(),
                location: "".to_string(),
            };
        }
    };
}

pub fn scrape_many(frag: &Html, select: &str) -> Vec<String> {
    let selector = Selector::parse(select).unwrap();
    let elems = frag.select(&selector);

    let mut results = Vec::new();
    for el in elems {
        results.push(el.inner_html());
    }

    results
}

pub fn scrape_many_outer(frag: &Html, select: &str) -> Vec<String> {
    let selector = Selector::parse(select).unwrap();
    let elems = frag.select(&selector);

    let mut results = Vec::new();
    for el in elems {
        results.push(el.html());
    }

    results
}

pub fn scrape_outer(frag: &Html, select: &str) -> String {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => return el.inner_html(),
        None => return "#####".to_string(),
    };
}

pub fn scrape_inner(frag: String, select: &str) -> String {
    let html = Html::parse_fragment(&frag);
    let selector = Selector::parse(select).unwrap();
    html.select(&selector).next().unwrap().html()
}
