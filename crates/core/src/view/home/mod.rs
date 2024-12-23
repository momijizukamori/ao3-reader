use std::collections::BTreeMap;

use url::Url;
use crate::font::Fonts;
use crate::view::{View, Event, Hub, Bus, RenderQueue, ViewId, Id, ID_FEEDER, RenderData};
use crate::view::{THICKNESS_MEDIUM, SMALL_BAR_HEIGHT, BIG_BAR_HEIGHT};
use crate::view::keyboard::Layout;
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
use crate::view::fave::Fave;
use crate::battery::Battery;

#[cfg(test)]
mod tests;

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

    fn open_search_bar(&mut self, keyboard_layouts: &BTreeMap<String, Layout>, keyboard_name: String, rq: &mut RenderQueue) {
        // TODO - remove when components determine own height
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (_small_thickness, big_thickness) = halves(thickness);

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
        let keyboard = Keyboard::new(&mut kb_rect, false, keyboard_layouts, keyboard_name);
        self.children.insert(index - 1, Box::new(keyboard) as Box<dyn View>);

        let keyboard_pos = self.children[rlocate::<Keyboard>(self).unwrap()].rect().clone();

        // add search bar child
        let search_rect = rect![
            self.rect.min.x, self.rect.min.y,
            self.rect.max.x, keyboard_pos.min.y];
        let search_bar = SearchBar::new(search_rect,
            ViewId::SiteTextSearchInput, "Search Ao3");
        self.children.insert(self.shelf_index+1, Box::new(search_bar) as Box<dyn View>);

        // Update the GUI with the new children
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    fn close_search_bar(&mut self, rq: &mut RenderQueue) {
        if let Some(index) = rlocate::<SearchBar>(self) {
            self.children.remove(index);
        }

        if let Some(index) = rlocate::<Keyboard>(self) {
            self.children.remove(index);
        }

        self.query = None;

        // Update the GUI with the removed children
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    fn reseed(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
            if let Some(top_bar) = self.child_mut(1).downcast_mut::<TopBar>() {
                top_bar.update_frontlight_icon(&mut RenderQueue::new(), context);
                hub.send(Event::ClockTick).ok();
                hub.send(Event::BatteryTick).ok();
            }

            rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
        }

    fn handle_search_events(&mut self, evt: &Event, keyboard_layouts: &BTreeMap<String, Layout>, keyboard_name: String, rq: &mut RenderQueue, hub: &Hub) -> bool {
        match *evt {
            Event::Toggle(ViewId::SearchBar) => {
                if let Some(_index) = rlocate::<SearchBar>(self) {
                    self.close_search_bar(rq);
                } else {
                    self.open_search_bar(keyboard_layouts, keyboard_name, rq);
                    // Focus the text input on the search bar
                    hub.send(Event::Focus(Some(ViewId::SiteTextSearchInput))).ok();
                }
                true
            },
            Event::ToggleNear(ViewId::SearchMenu, _rect) => {
                hub.send(Event::SubmitInput(ViewId::SiteTextSearchInput)).ok();
                true
            },
            Event::Close(ViewId::SearchBar) => {
                self.close_search_bar(rq);
                true
            },
            Event::Submit(ViewId::SiteTextSearchInput, ref text) => {
                self.close_search_bar(rq);
                hub.send(Event::LoadSearch(text.to_string())).ok();
                true
            },
            Event::Focus(v) => {
                if self.focus != v {
                    self.focus = v;
                }
                true
            },
            _ => false
        }
    }
}

impl View for Home {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        if self.handle_search_events(evt, &context.keyboard_layouts, context.settings.keyboard_layout.clone(), rq, hub) {
            return true;
        }

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
