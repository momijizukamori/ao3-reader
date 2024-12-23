use std::collections::HashMap;

use crate::color::BLACK;
use crate::device::CURRENT_DEVICE;
use crate::font::LABEL_STYLE;
use crate::geom::Rectangle;
use crate::unit::scale_by_dpi;
use crate::view::{ID_FEEDER, MINI_BAR_HEIGHT, SMALL_PADDING, THICKNESS_MEDIUM};
use crate::view::{View, Event, Id, Align};
use crate::view::filler::Filler;
use crate::view::textlabel::TextLabel;

// Children names for lookup
pub const LABEL: &str = "label";
pub const BORDER: &str = "border";

#[derive(Debug, Clone)]
pub struct Fave {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    content_height: i32,
    border_thickness: i32,
    children_lookup: HashMap<String, usize>
}

impl Fave {
    pub fn new_empty(parent_rect: Rectangle, offset: i32) -> Fave {
        let dpi = CURRENT_DEVICE.dpi;

        let content_height = scale_by_dpi(MINI_BAR_HEIGHT, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        Fave {
            id: ID_FEEDER.next(),
            rect: rect![
                parent_rect.min.x, parent_rect.min.y + offset,
                parent_rect.max.x, parent_rect.min.y + offset + content_height + border_thickness],
            children: Vec::new(),
            content_height,
            border_thickness,
            children_lookup: HashMap::new()
        }
    }
    pub fn new(parent_rect: Rectangle, offset:i32, title: String, root_event: Event) -> Fave {
        let mut fave = Fave::new_empty(parent_rect, offset);

        fave.create_label(title, root_event);
        fave.create_border();

        fave
    }

    fn create_label(&mut self, title: String, event: Event) {
        let dpi = CURRENT_DEVICE.dpi;
        let label_padding = scale_by_dpi(SMALL_PADDING, dpi) as i32;

        let label_rect = rect![
                self.rect.min.x, self.rect.min.y,
                self.rect.max.x, self.rect.min.y + self.content_height];

        println!("Creating label with text: {}", title);

        let chapter = TextLabel::new(label_rect, title,
            Align::Left(label_padding), LABEL_STYLE, event);
        self.children_lookup.insert(LABEL.to_string(), self.children.len());
        self.children.push(Box::new(chapter) as Box<dyn View>);
    }

    fn create_border(&mut self) {
        let seperator_rect = rect![
            self.rect.min.x, self.rect.max.y - self.border_thickness,
            self.rect.max.x, self.rect.max.y];
        let seperator = Filler::new(seperator_rect, BLACK);

        self.children_lookup.insert(BORDER.to_string(), self.children.len());
        self.children.push(Box::new(seperator) as Box<dyn View>);
    }
}

// TODO - figure out a way to move these to the view level
// Currently they have limited ode coverage and high
// repition
impl View for Fave {
    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }

    fn id(&self) -> Id {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ViewId;

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_createLabelIsCalled_THEN_aLeftAlignedLabelIsCreated() {
        // WHEN create_label is called with Search Event
        let width = 300;
        let height = 67;
        let mut fave = Fave::new_empty(rect![0, 0, width, height], 0);
        fave.create_label("Fake Fave".to_string(), Event::Toggle(ViewId::SearchBar));
        // THEN a left-aligned Label is created
        assert_eq!(fave.children.len(), 1);
        assert_eq!(fave.children[0].rect(), &rect![0, 0, width, 56]);
        let _label = fave.child_mut(0).downcast_mut::<TextLabel>().unwrap();
    }

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_createBorderIsCalled_THEN_aFillerIsCreated() {
        // WHEN create_label is called with Search Event
        let width = 300;
        let height = 67;
        let mut fave = Fave::new_empty(rect![0, 0, width, height], 0);
        fave.create_border();
        // THEN a left-aligned Label is created
        assert_eq!(fave.children.len(), 1);
        assert_eq!(fave.children[0].rect(), &rect![0, 56, width, 57]);
        let _border = fave.child_mut(0).downcast_mut::<Filler>().unwrap();
    }


    #[test]
    #[allow(non_snake_case)]
    fn WHEN_faveNewIsCalled_THEN_aFaveWithTheStandardChildrenIsCreated() {
        // WHEN Fave::new() is called
        let fave = Fave::new(rect![0, 0, 600, 800], 5, "Test Fave".to_string(), Event::Toggle(ViewId::SearchBar));

        // THEN a ave with the standard children is created
        assert_eq!(fave.children_lookup, HashMap::from([
            (LABEL.to_string(), 0usize),
            (BORDER.to_string(), 1usize)
        ]));
    }
}
