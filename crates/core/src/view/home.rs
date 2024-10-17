use url::Url;
use crate::font::Fonts;
use crate::view::{View, Event, Hub, Bus, RenderQueue, ViewId, Id, ID_FEEDER, RenderData};
use crate::view::{THICKNESS_MEDIUM, SMALL_BAR_HEIGHT, BIG_BAR_HEIGHT};
use crate::context::Context;
use crate::unit::scale_by_dpi;
use crate::geom::{halves, Rectangle};
use crate::color::{BLACK, WHITE};
use crate::device::CURRENT_DEVICE;
use crate::framebuffer::UpdateMode;
use crate::view::filler::Filler;
use crate::view::common::{locate, toggle_main_menu, toggle_battery_menu, toggle_clock_menu, rlocate};
use super::top_bar::TopBar;
use super::bottom_bar::BottomBar;
use crate::view::keyboard::Keyboard;
use crate::view::search_bar::SearchBar;
use crate::view::notification::Notification;
use crate::view::fave::Fave;
use crate::battery::Battery;

// Children names for lookup
pub const BACKGROUND: &str = "background";
pub const TOP_BAR: &str = "top_bar";
pub const MARKED_FOR_LATER: &str = "marked_for_later";
pub const FAVES: &str = "faves";
pub const SEARCH_BAR: &str = "bottom_bar";
pub const KEYBOARD: &str = "bottom_bar";
pub const BOTTOM_BAR: &str = "bottom_bar";

#[derive(Clone)]
pub struct Home {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    id: Id,
    view_id: ViewId,
    shelf_index: usize,
    focus: Option<ViewId>,
    query: Option<String>
}

impl Home {
    pub fn new_empty(rect: Rectangle) -> Home {
        let id = ID_FEEDER.next();
        let children = Vec::new();
    
        Home {
            rect,
            children,
            id,
            view_id: ViewId::Home,
            shelf_index: 0,
            query: None,
            focus: None
        }
    }


    pub fn new(rect: Rectangle, rq: &mut RenderQueue,
               format: String, fonts: &mut Fonts, battery: &mut Box<dyn Battery>, frontlight: bool, logged_in: bool, faves: &Vec<(String, Url)>) -> Home {
        let mut home = Home::new_empty(rect);

        home.create_background();

        home.create_top_bar(format, fonts, battery, frontlight);
        let top_bar = &home.children[rlocate::<TopBar>(&home).unwrap()];

        // TODO add login/logged in section

        let mut top_pos = top_bar.rect().height() as i32;

        // Link to 'Marked for Later' view
        if logged_in {
            home.create_marked_for_later(top_pos);
            top_pos = home.children[home.children.len() - 1].rect().max.y;
        }

        // TODO - make this actually the bottom bar after refactoring search to not be
        // so heavily tied to indexes :(
        let bottom_bar_top = home.rect().min.y;
        let mut fav_index = 0;
        while fav_index < faves.len() {
            home.create_fav_search(faves[fav_index].clone(), top_pos);
            top_pos = home.children[home.children.len() - 1].rect().max.y;
            let row_height = home.children[home.children.len() - 1].rect().height() as i32;
            fav_index = fav_index + 1;

            // If the next fave would overlap wth the bottom bar, we should not create
            // any more faves
            if top_pos + row_height > bottom_bar_top { break };
        }

        home.set_shelf_index(home.children.len() - 1); 
        home.create_bottom_bar();
        rq.add(RenderData::new(home.id, rect, UpdateMode::Full));
        home
    }

    fn create_background(&mut self) {
        let bg = Filler::new(self.rect, WHITE);
        self.children.push(Box::new(bg) as Box<dyn View>);
    }

    fn create_top_bar(&mut self, format: String, fonts: &mut Fonts, battery: &mut Box<dyn Battery>, frontlight: bool) {
        let top_bar = TopBar::new(self.rect,
                                  Event::Toggle(ViewId::SearchBar),
                                  "Favorite Tags".to_string(),
                                  format, fonts, battery, frontlight);
        self.children.push(Box::new(top_bar) as Box<dyn View>);
    }

    fn create_bottom_bar(&mut self) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let small_height= scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);

        let separator = Filler::new(rect![self.rect.min.x, self.rect.max.y - small_height - small_thickness,
            self.rect.max.x, self.rect.max.y - small_height + big_thickness], BLACK);
        self.children.push(Box::new(separator) as Box<dyn View>);
        // TODO: should eventually actually allow flipping through pages, if there are more favorites than will fit on one page
        let bottom_bar = BottomBar::new(rect![self.rect.min.x, self.rect.max.y - small_height + big_thickness,
            self.rect.max.x, self.rect.max.y], 0, 1);
        self.children.push(Box::new(bottom_bar) as Box<dyn View>);
    }

    fn create_marked_for_later(&mut self, top_pos: i32) {
        let marked_for_later = Fave::new(
            self.rect, top_pos,
            "Marked For Later".to_string(),
            Event::LoadHistory(super::works::HistoryView::MarkedForLater));

        self.children.push(Box::new(marked_for_later) as Box<dyn View>);
    }

    fn create_fav_search(&mut self, fave: (String, Url), top_pos: i32) {
        let fave = Fave::new(
            self.rect, top_pos,
            (*fave.0).to_string(),
            Event::LoadIndex((fave.1).to_string()));

        self.children.push(Box::new(fave) as Box<dyn View>);
    }

    fn set_shelf_index(&mut self, index: usize) {
        self.shelf_index = index;
    }

    fn open_search_bar(&mut self, context: &mut Context) {
        // TODO - remove when components determine own height
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let delta_y = small_height;

        // search bar should be bottom-aligned, but not cover the bottom bar
        // So we need to know the top y pos of the bottom bar
        let index = rlocate::<BottomBar>(self).unwrap();
        let bottom_bar = &self.children[index];

        // add keyboard child - based on research Kobos do not support physical keyboards
        // without extensive technical setup, so we should assume that we always need to
        // display the keyboard when we display the search input
        let mut kb_rect = rect![
            // TODO - figure out a less arbitrary min y for keyboard
            self.rect.min.x, bottom_bar.rect().min.y - (small_height + 3 * big_height) as i32 + big_thickness,
            self.rect.max.x, bottom_bar.rect().min.y];
        let keyboard = Keyboard::new(&mut kb_rect, false, &context.keyboard_layouts, context.settings.keyboard_layout.clone());
        self.children.insert(index - 1, Box::new(keyboard) as Box<dyn View>);

        let keyboard_pos = self.children[rlocate::<Keyboard>(self).unwrap()].rect().clone();

        // TODO - add top border seperator to keyboard element instead of as seperate item
        let separator = Filler::new(rect![
            self.rect.min.x, keyboard_pos.min.y - thickness,
            self.rect.max.x, keyboard_pos.min.y], BLACK);
        self.children.insert(index - 1, Box::new(separator) as Box<dyn View>);

        // add search bar child
        let mut search_rect = rect![
            self.rect.min.x, keyboard_pos.min.y - small_height,
            self.rect.max.x, keyboard_pos.min.y];
        let search_bar = SearchBar::new(search_rect,
            ViewId::SiteTextSearchInput, "Search Ao3");
        self.children.insert(self.shelf_index+1, Box::new(search_bar) as Box<dyn View>);

        // TODO Move to Search Bar implementation
        let separator = Filler::new(rect![
            self.rect.min.x, search_rect.min.y - thickness,
            self.rect.max.x, search_rect.min.y], BLACK);
        self.children.insert(self.shelf_index+1, Box::new(separator) as Box<dyn View>);
    }

    fn toggle_search_bar(&mut self, enable: Option<bool>, update: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let delta_y = small_height;
        let search_visible: bool;
        let mut has_keyboard = false;

        if let Some(index) = rlocate::<SearchBar>(self) {
            if let Some(true) = enable {
                return;
            }

            if let Some(ViewId::SiteTextSearchInput) = self.focus {
                self.toggle_keyboard(false, false, Some(ViewId::SiteTextSearchInput), hub, rq, context);
            }

            // Remove the search bar and its separator.
            self.children.drain(index - 1 ..= index);

            // Move the shelf's bottom edge.
            self.children[self.shelf_index].rect_mut().max.y += delta_y;

            self.query = None;
            search_visible = false;
        } else {
            if let Some(false) = enable {
                return;
            }

            self.open_search_bar(context);
            has_keyboard = true;

            if self.query.is_none() {
                hub.send(Event::Focus(Some(ViewId::SiteTextSearchInput))).ok();
            }

            search_visible = true;
        }

        if update {
            if !search_visible {
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
            }


            if search_visible {
                rq.add(RenderData::new(self.child(self.shelf_index-1).id(), *self.child(self.shelf_index-1).rect(), UpdateMode::Partial));
                let mut rect = *self.child(self.shelf_index).rect();
                rect.max.y = self.child(self.shelf_index+1).rect().min.y;
                // Render the part of the shelf that isn't covered.
                rq.add(RenderData::new(self.child(self.shelf_index).id(), rect, UpdateMode::Partial));
                // Render the views on top of the shelf.
                rect.min.y = rect.max.y;
                let end_index = self.shelf_index + if has_keyboard { 4 } else { 2 };
                rect.max.y = self.child(end_index).rect().max.y;
                rq.add(RenderData::expose(rect, UpdateMode::Partial));
            } else {
                for i in self.shelf_index - 1 ..= self.shelf_index + 1 {
                    if i == self.shelf_index {
                        continue;
                    }
                    rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Partial));
                }
            }

            // self.update_bottom_bar(rq);
        }
    }

    fn toggle_keyboard(&mut self, enable: bool, update: bool, id: Option<ViewId>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let has_search_bar = self.children[self.shelf_index+2].is::<SearchBar>();

        if let Some(index) = rlocate::<Keyboard>(self) {
            if enable {
                return;
            }

            let y_min = self.child(self.shelf_index+1).rect().min.y;
            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect());

            self.children.drain(index - 1 ..= index);

            let delta_y = rect.height() as i32;

            if has_search_bar {
                for i in self.shelf_index+1..=self.shelf_index+2 {
                    let shifted_rect = *self.child(i).rect() + pt!(0, delta_y);
                    self.child_mut(i).resize(shifted_rect, hub, rq, context);
                }
            }

            context.kb_rect = Rectangle::default();
            hub.send(Event::Focus(None)).ok();
            if update {
                let rect = rect![self.rect.min.x, y_min,
                                 self.rect.max.x, y_min + delta_y];
                rq.add(RenderData::expose(rect, UpdateMode::Gui));
            }
        } else {
            if !enable {
                return;
            }

            // Technically this also adds the search bar in to children,
            // but this is temporary until I can nuke these toggle functions
            // so Iiiii thnk its fine actually
            self.open_search_bar(context);
        }

        if update {
            if enable {
                if has_search_bar {
                    for i in self.shelf_index+1..=self.shelf_index+4 {
                        let update_mode = if (i - self.shelf_index) == 1 { UpdateMode::Partial } else { UpdateMode::Gui };
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), update_mode));
                    }
                } else {
                    for i in self.shelf_index+1..=self.shelf_index+2 {
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                    }
                }
            } else if has_search_bar {
                for i in self.shelf_index+1..=self.shelf_index+2 {
                    rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                }
            }
        }
    }

    fn reseed(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
            if let Some(top_bar) = self.child_mut(1).downcast_mut::<TopBar>() {
                top_bar.update_frontlight_icon(&mut RenderQueue::new(), context);
                hub.send(Event::ClockTick).ok();
                hub.send(Event::BatteryTick).ok();
            }
    
            rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
        }

}

impl View for Home {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Reseed => {
                self.reseed(hub, rq, context);
                true
            },
            Event::ToggleFrontlight => {
                if let Some(index) = locate::<TopBar>(self) {
                    self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                        .update_frontlight_icon(rq, context);
                }
                true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => {
                toggle_battery_menu(self, rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::ClockMenu, rect) => {
                toggle_clock_menu(self, rect, None, rq, context);
                true
            },
            Event::Close(ViewId::MainMenu) => {
                toggle_main_menu(self, Rectangle::default(), Some(false), rq, context);
                true
            },

            // Ao3 Text Search
            Event::Toggle(ViewId::SearchBar) => {
                self.toggle_search_bar(None, true, hub, rq, context);
                true
            },
            Event::ToggleNear(ViewId::SearchMenu, _rect) => {
                hub.send(Event::SubmitInput(ViewId::SiteTextSearchInput)).ok();
                true
            },
            Event::Close(ViewId::SearchBar) => {
                self.toggle_search_bar(Some(false), true, hub, rq, context);
                true
            },
            Event::Submit(ViewId::SiteTextSearchInput, ref text) => {
                self.query = Some(text.to_string());
                if self.query.is_some() {
                    self.toggle_keyboard(false, false, None, hub, rq, context);
                    self.toggle_search_bar(Some(false), false, hub, rq, context);
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                    hub.send(Event::LoadSearch(text.to_string())).ok();
                } else {
                    let notif = Notification::new("Invalid search query.".to_string(),
                                                  hub, rq, context);
                    self.children.push(Box::new(notif) as Box<dyn View>);
                }
                true
            },

            // This does some sort of unintuitive cleanup with the keyboard :(
            // The only parts of the code that send this are the toggle_search_bar
            // and toggle_keyboard above, and the input field when it is tapped
            // I suspect it is primarily designed to show/hide just the keyboard
            // when the input field is focused, but we want to treat the search as
            // a single modal, and show hide the keyboard and input simultaniously
            Event::Focus(v) => {
                if self.focus != v {
                    self.focus = v;
                    if v.is_some() {
                        self.toggle_keyboard(true, true, v, hub, rq, context);
                    }
                }
                true
            },
            _ => false
        }
    }

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

    fn view_id(&self) -> Option<ViewId> {
        Some(self.view_id)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::FakeBattery;

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_createBackgroundIsCalled_THEN_aFullSizeWhiteRectangleIsAddedToChildren() {
        // WHEN create_background is called
        let mut home = Home::new_empty(rect![0, 0, 600, 800]);
        home.create_background();
        // THEN a full size white rectangle is added to children
        assert_eq!(home.children.len(), 1);
        assert_eq!(home.children[0].rect(), &rect![0, 0, 600, 800]);
        let _test_type = home.child_mut(0).downcast_mut::<Filler>().unwrap();
    }

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_createTopBarIsCalled_THEN_aTopBarIsAddedToChildren() {
        // WHEN create_top_bar is called
        let mut home = Home::new_empty(rect![0, 0, 600, 800]);
        let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
        home.create_top_bar("%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(), &mut battery, true);
        // THEN a top bar is added to children
        assert_eq!(home.children.len(), 1);
        assert_eq!(home.children[0].rect(), &rect![0, 0, 600, 68]);
        let _test_type = home.child_mut(0).downcast_mut::<TopBar>().unwrap();
    }

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_createMarkedForLaterIsCalled_THEN_aMarkedForLaterLabelIsAddedToChildren() {
        // WHEN create_marked_for_later is called
        let mut home = Home::new_empty(rect![0, 0, 600, 800]);
        home.create_marked_for_later(5);
        // THEN a marked for later label is added to children
        assert_eq!(home.children.len(), 1);
        assert_eq!(home.children[0].rect(), &rect![0, 5, 600, 62]);
        let _label = home.child_mut(0).downcast_mut::<Fave>().unwrap();
    }

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_createFaveSearchIsCalled_THEN_aFaveLabelIsAddedToChildren() {
        // WHEN create_marked_for_later is called
        let mut home = Home::new_empty(rect![0, 0, 600, 800]);
        home.create_fav_search(("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL")), 5);
        // THEN a marked for later label is added to children
        assert_eq!(home.children.len(), 1);
        assert_eq!(home.children[0].rect(), &rect![0, 5, 600, 62]);
        let _label = home.child_mut(0).downcast_mut::<Fave>().unwrap();
    }

    #[test]
    #[allow(non_snake_case)]
    fn GIVEN_loggedInUser_WHEN_homeNewIsCalled_THEN_aHomePageWithTheStandardChildrenPlusMarkedForLaterIsCreated() {
        // WHEN Home::new() is called
        let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
        let mut rq = RenderQueue::new();
        let home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                  &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);

        // THEN a home with the standard children plus a marked for later fave is called
        assert_eq!(locate::<Filler>(&home).unwrap(), 0);
        assert_eq!(locate::<TopBar>(&home).unwrap(), 1);
        assert_eq!(locate::<Fave>(&home).unwrap(), 2); // marked for later
        assert_eq!(rlocate::<Fave>(&home).unwrap(), 3); // test fave
        assert_eq!(rlocate::<BottomBar>(&home).unwrap(), 5);
    }
}
