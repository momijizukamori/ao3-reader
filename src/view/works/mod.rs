mod title_bar;
mod works_label;
pub mod work;
pub mod workindex;
mod bottom_bar;

use std::fs;
use std::path::{Path, PathBuf};
use rand_core::RngCore;
use anyhow::{Error, format_err};
use crate::library::Library;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::metadata::{Metadata, SortMethod, BookQuery, SimpleStatus, sort};
use crate::view::{View, Event, Hub, Bus, RenderQueue, RenderData};
use crate::view::{Id, ID_FEEDER, ViewId, EntryId, EntryKind};
use crate::view::{SMALL_BAR_HEIGHT, BIG_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::settings::{Hook, LibraryMode, FirstColumn, SecondColumn};
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::view::common::{locate, rlocate, locate_by_id};
use crate::view::filler::Filler;
use crate::view::keyboard::Keyboard;
use crate::view::named_input::NamedInput;
use crate::view::menu::{Menu, MenuKind};
use crate::view::menu_entry::MenuEntry;
use crate::view::search_bar::SearchBar;
use crate::view::notification::Notification;
use super::top_bar::TopBar;
use self::workindex::WorkIndex;
use self::bottom_bar::BottomBar;
use self::title_bar::TitleBar;
use crate::gesture::GestureEvent;
use crate::geom::{Rectangle, Dir, CycleDir, halves};
use crate::input::{DeviceEvent, ButtonCode, ButtonStatus};
use crate::device::CURRENT_DEVICE;
use crate::unit::scale_by_dpi;
use crate::color::BLACK;
use crate::font::Fonts;
use crate::app::Context;

pub const TRASH_DIRNAME: &str = ".trash";

#[derive(Debug, Clone)]
pub struct Works {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    current_page: usize,
    pages_count: usize,
    works_count: usize,
    works_lines: usize,
    shelf_index: usize,
    focus: Option<ViewId>,
    query: Option<BookQuery>,
    sort_method: SortMethod,
    reverse_order: bool,
    visible_books: Metadata,
    current_directory: PathBuf,
}

impl Works {
    pub fn new(rect: Rectangle, index_url: String, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Result<Works, Error> {
        let id = ID_FEEDER.next();
        let dpi = CURRENT_DEVICE.dpi;
        let mut children = Vec::new();

        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);

        let selected_library = context.settings.selected_library;
        let library_settings = &context.settings.libraries[selected_library];

        let current_directory = context.library.home.clone();
        let sort_method = library_settings.sort_method;
        let reverse_order = sort_method.reverse_order();

        context.library.sort(sort_method, reverse_order);

        let (visible_books, dirs) = context.library.list(&current_directory, None, false);
        let mut shelf_index = 1;

        let top_bar = TopBar::new(rect![rect.min.x, rect.min.y,
                                        rect.max.x, rect.min.y + small_height + big_thickness],
                                  Event::Toggle(ViewId::SearchBar),
                                  sort_method.title(),
                                  context);
        children.push(Box::new(top_bar) as Box<dyn View>);

        let mut y_start = rect.min.y + small_height + big_thickness;

        let mut workindex = WorkIndex::new(rect![rect.min.x, y_start,
                                         rect.max.x, rect.max.y],
                                   false,
                                   index_url,
                                   hub,
                                   context);

        workindex.get_works(context, &mut RenderQueue::new());

        let current_page = workindex.current_page;
        let pages_count = workindex.max_page;
        let works_count = workindex.max_works;
        let works_lines = workindex.max_lines;

        children.push(Box::new(workindex) as Box<dyn View>);

        rq.add(RenderData::new(id, rect, UpdateMode::Full));

        Ok(Works {
            id,
            rect,
            children,
            current_page,
            pages_count,
            shelf_index,
            focus: None,
            query: None,
            sort_method,
            reverse_order,
            visible_books,
            current_directory,
            works_count,
            works_lines
        })
    }

    fn go_to_page(&mut self, index: usize, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if index >= self.pages_count {
            return;
        }
        self.current_page = index;
        self.update_shelf(false, hub, rq, context);
        self.update_bottom_bar(rq);
    }

    fn go_to_neighbor(&mut self, dir: CycleDir, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        match dir {
            CycleDir::Next if self.current_page < self.pages_count.saturating_sub(1) => {
                self.current_page += 1;
            },
            CycleDir::Previous if self.current_page > 0 => {
                self.current_page -= 1;
            },
            _ => return,
        }

        self.update_shelf(false, hub, rq, context);
        self.update_bottom_bar(rq);
    }

    // NOTE: This function assumes that the workindex wasn't resized.
    fn refresh_visibles(&mut self, update: bool, reset_page: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let (files, _) = context.library.list(&self.current_directory,
                                              self.query.as_ref(),
                                              false);
        self.visible_books = files;

        let workindex = self.child(self.shelf_index).downcast_ref::<WorkIndex>().unwrap();

        if reset_page  {
            self.current_page = 0;
        } else if self.current_page >= self.pages_count {
            self.current_page = self.pages_count.saturating_sub(1);
        }

        if update {
            self.update_shelf(false, hub, rq, context);
            self.update_bottom_bar(rq);
        }
    }

    fn update_thumbnail_previews(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let selected_library = context.settings.selected_library;
        self.children[self.shelf_index].as_mut().downcast_mut::<WorkIndex>().unwrap()
           .set_thumbnail_previews(context.settings.libraries[selected_library].thumbnail_previews);
        self.update_shelf(false, hub, rq, context);
    }

    fn update_shelf(&mut self, was_resized: bool, _hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let workindex = self.children[self.shelf_index].as_mut().downcast_mut::<WorkIndex>().unwrap();
        let max_lines = ((workindex.rect.height() as i32 + thickness) / big_height) as usize;

        if was_resized {
            let page_position = if self.visible_books.is_empty() {
                0.0
            } else {
                self.current_page as f32 * (workindex.max_lines as f32 /
                                            self.visible_books.len() as f32)
            };

            let mut page_guess = page_position * self.visible_books.len() as f32 / max_lines as f32;
            let page_ceil = page_guess.ceil();

            if (page_ceil - page_guess) < f32::EPSILON {
                page_guess = page_ceil;
            }

            self.pages_count = workindex.max_page;
            self.works_count = workindex.max_works;
            self.works_lines = workindex.max_lines;
            self.current_page = (page_guess as usize).min(self.pages_count.saturating_sub(1));
        }

        workindex.set_page(self.current_page);
        workindex.get_works(context, rq);
    }

    fn update_top_bar(&mut self, search_visible: bool, rq: &mut RenderQueue) {
        if let Some(index) = locate::<TopBar>(self) {
            let top_bar = self.children[index].as_mut().downcast_mut::<TopBar>().unwrap();
            let name = if search_visible { "back" } else { "search" };
            top_bar.update_root_icon(name, rq);
            top_bar.update_title_label(&self.sort_method.title(), rq);
        }
    }

    fn update_bottom_bar(&mut self, rq: &mut RenderQueue) {
        // if let Some(index) = rlocate::<BottomBar>(self) {
        //     let bottom_bar = self.children[index].as_mut().downcast_mut::<BottomBar>().unwrap();
        //     bottom_bar.update_works_label(self.current_page, self.works_count, self.works_lines, rq);
        //     bottom_bar.update_page_label(self.current_page, self.pages_count, rq);
        //     bottom_bar.update_icons(self.current_page, self.pages_count, rq);
        // }
    }

    fn toggle_keyboard(&mut self, enable: bool, update: bool, id: Option<ViewId>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let has_search_bar = self.children[self.shelf_index+1].is::<SearchBar>();

        if let Some(index) = rlocate::<Keyboard>(self) {
            if enable {
                return;
            }

            let y_min = self.child(self.shelf_index+1).rect().min.y;
            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect());

            self.children.drain(index - 1 ..= index);

            let delta_y = rect.height() as i32;

            // if has_search_bar {
            //     for i in self.shelf_index+1..=self.shelf_index+2 {
            //         let shifted_rect = *self.child(i).rect() + pt!(0, delta_y);
            //         self.child_mut(i).resize(shifted_rect, hub, rq, context);
            //     }
            // }

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

            //let index = rlocate::<BottomBar>(self).unwrap() - 1;
            let mut kb_rect = rect![self.rect.min.x,
                                    self.rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    self.rect.max.x,
                                    self.rect.max.y - small_height - small_thickness];

            let number = match id {
                Some(ViewId::GoToPageInput) => true,
                _ => false,
            };

            let keyboard = Keyboard::new(&mut kb_rect, number, context);
            self.children.push(Box::new(keyboard) as Box<dyn View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.push(Box::new(separator) as Box<dyn View>);

            let delta_y = kb_rect.height() as i32 + thickness;

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

    fn toggle_search_bar(&mut self, enable: Option<bool>, update: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        // let dpi = CURRENT_DEVICE.dpi;
        // let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
        //                                   scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        // let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        // let delta_y = small_height;
        // let search_visible: bool;
        // let mut has_keyboard = false;

        // if let Some(index) = rlocate::<SearchBar>(self) {
        //     if let Some(true) = enable {
        //         return;
        //     }

        //     if let Some(ViewId::HomeSearchInput) = self.focus {
        //         self.toggle_keyboard(false, false, Some(ViewId::HomeSearchInput), hub, rq, context);
        //     }

        //     // Remove the search bar and its separator.
        //     self.children.drain(index - 1 ..= index);

        //     // Move the workindex's bottom edge.
        //     self.children[self.shelf_index].rect_mut().max.y += delta_y;

        //     self.query = None;
        //     search_visible = false;
        // } else {
        //     if let Some(false) = enable {
        //         return;
        //     }

        //     let sp_rect = *self.child(self.shelf_index+1).rect() - pt!(0, delta_y);
        //     let search_bar = SearchBar::new(rect![self.rect.min.x, sp_rect.max.y,
        //                                           self.rect.max.x,
        //                                           sp_rect.max.y + delta_y - thickness],
        //                                     ViewId::HomeSearchInput,
        //                                     "Title, author, series",
        //                                     "", context);
        //     self.children.insert(self.shelf_index+1, Box::new(search_bar) as Box<dyn View>);

        //     let separator = Filler::new(sp_rect, BLACK);
        //     self.children.insert(self.shelf_index+1, Box::new(separator) as Box<dyn View>);

        //     // Move the workindex's bottom edge.
        //     self.children[self.shelf_index].rect_mut().max.y -= delta_y;

        //     if self.query.is_none() {
        //         if rlocate::<Keyboard>(self).is_none() {
        //             self.toggle_keyboard(true, false, Some(ViewId::HomeSearchInput), hub, rq, context);
        //             has_keyboard = true;
        //         }

        //         hub.send(Event::Focus(Some(ViewId::HomeSearchInput))).ok();
        //     }

        //     search_visible = true;
        // }

        // if update {
        //     if !search_visible {
        //         self.refresh_visibles(false, true, hub, rq, context);
        //     }

        //     self.update_top_bar(search_visible, rq);

        //     if search_visible {
        //         rq.add(RenderData::new(self.child(self.shelf_index-1).id(), *self.child(self.shelf_index-1).rect(), UpdateMode::Partial));
        //         let mut rect = *self.child(self.shelf_index).rect();
        //         rect.max.y = self.child(self.shelf_index+1).rect().min.y;
        //         // Render the part of the workindex that isn't covered.
        //         self.update_shelf(true, hub, &mut RenderQueue::new(), context);
        //         rq.add(RenderData::new(self.child(self.shelf_index).id(), rect, UpdateMode::Partial));
        //         // Render the views on top of the workindex.
        //         rect.min.y = rect.max.y;
        //         let end_index = self.shelf_index + if has_keyboard { 4 } else { 2 };
        //         rect.max.y = self.child(end_index).rect().max.y;
        //         rq.add(RenderData::expose(rect, UpdateMode::Partial));
        //     } else {
        //         for i in self.shelf_index - 1 ..= self.shelf_index + 1 {
        //             if i == self.shelf_index {
        //                 self.update_shelf(true, hub, rq, context);
        //                 continue;
        //             }
        //             rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Partial));
        //         }
        //     }

        //     self.update_bottom_bar(rq);
        // }
    }

    fn toggle_go_to_page(&mut self, enable: Option<bool>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::GoToPage) {
            if let Some(true) = enable {
                return;
            }
            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
            if let Some(ViewId::GoToPageInput) = self.focus {
                self.toggle_keyboard(false, true, Some(ViewId::GoToPageInput), hub, rq, context);
            }
        } else {
            if let Some(false) = enable {
                return;
            }
            if self.pages_count < 2 {
                return;
            }
            let go_to_page = NamedInput::new("Go to page".to_string(),
                                             ViewId::GoToPage,
                                             ViewId::GoToPageInput,
                                             4, context);
            rq.add(RenderData::new(go_to_page.id(), *go_to_page.rect(), UpdateMode::Gui));
            hub.send(Event::Focus(Some(ViewId::GoToPageInput))).ok();
            self.children.push(Box::new(go_to_page) as Box<dyn View>);
        }
    }

    fn toggle_sort_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SortMenu) {
            if let Some(true) = enable {
                return;
            }
            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let entries = vec![EntryKind::RadioButton("Date Opened".to_string(),
                                                      EntryId::Sort(SortMethod::Opened),
                                                      self.sort_method == SortMethod::Opened),
                               EntryKind::RadioButton("Date Added".to_string(),
                                                      EntryId::Sort(SortMethod::Added),
                                                      self.sort_method == SortMethod::Added),
                               EntryKind::RadioButton("Progress".to_string(),
                                                      EntryId::Sort(SortMethod::Progress),
                                                      self.sort_method == SortMethod::Progress),
                               EntryKind::RadioButton("Author".to_string(),
                                                      EntryId::Sort(SortMethod::Author),
                                                      self.sort_method == SortMethod::Author),
                               EntryKind::RadioButton("Title".to_string(),
                                                      EntryId::Sort(SortMethod::Title),
                                                      self.sort_method == SortMethod::Title),
                               EntryKind::RadioButton("Year".to_string(),
                                                      EntryId::Sort(SortMethod::Year),
                                                      self.sort_method == SortMethod::Year),
                               EntryKind::RadioButton("File Size".to_string(),
                                                      EntryId::Sort(SortMethod::Size),
                                                      self.sort_method == SortMethod::Size),
                               EntryKind::RadioButton("File Type".to_string(),
                                                      EntryId::Sort(SortMethod::Kind),
                                                      self.sort_method == SortMethod::Kind),
                               EntryKind::RadioButton("File Name".to_string(),
                                                      EntryId::Sort(SortMethod::FileName),
                                                      self.sort_method == SortMethod::FileName),
                               EntryKind::RadioButton("File Path".to_string(),
                                                      EntryId::Sort(SortMethod::FilePath),
                                                      self.sort_method == SortMethod::FilePath),
                               EntryKind::Separator,
                               EntryKind::CheckBox("Reverse Order".to_string(),
                                                   EntryId::ReverseOrder, self.reverse_order)];
            let sort_menu = Menu::new(rect, ViewId::SortMenu, MenuKind::DropDown, entries, context);
            rq.add(RenderData::new(sort_menu.id(), *sort_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(sort_menu) as Box<dyn View>);
        }
    }

    fn set_status(&mut self, path: &Path, status: SimpleStatus, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        // context.library.set_status(path, status);

        // // Is the current sort method affected by this change?
        // if self.sort_method == SortMethod::Progress ||
        //    self.sort_method == SortMethod::Opened {
        //     self.sort(false, hub, rq, context);
        // }

        // self.refresh_visibles(true, false, hub, rq, context);
    }

    fn remove(&mut self, path: &Path, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Result<(), Error> {
        let trash_path = context.library.home.join(TRASH_DIRNAME);
        if !trash_path.is_dir() {
            fs::create_dir_all(&trash_path)?;
        }
        let mut trash = Library::new(trash_path, LibraryMode::Database);
        context.library.move_to(path, &mut trash)?;
        let full_path = context.library.home.join(path);
        context.settings.intermission_images.retain(|_, path| path != &full_path);
        let (mut files, _) = trash.list(&trash.home, None, false);
        let mut size = files.iter().map(|info| info.file.size).sum::<u64>();
        if size > context.settings.home.max_trash_size {
            sort(&mut files, SortMethod::Added, true);
            while size > context.settings.home.max_trash_size {
                let info = files.pop().unwrap();
                if let Err(e) = trash.remove(&info.file.path) {
                    eprintln!("{}", e);
                    break;
                }
                size -= info.file.size;
            }
        }
        trash.flush();
        self.refresh_visibles(true, false, hub, rq, context);
        Ok(())
    }

    fn set_reverse_order(&mut self, value: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.reverse_order = value;
        self.current_page = 0;
        self.sort(true, hub, rq, context);
    }

    fn set_sort_method(&mut self, sort_method: SortMethod, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.sort_method = sort_method;
        self.reverse_order = sort_method.reverse_order();

        if let Some(index) = locate_by_id(self, ViewId::SortMenu) {
            self.child_mut(index)
                .children_mut().last_mut().unwrap()
                .downcast_mut::<MenuEntry>().unwrap()
                .update(sort_method.reverse_order(), rq);
        }

        self.current_page = 0;
        self.sort(true, hub, rq, context);
    }

    fn sort(&mut self, update: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        context.library.sort(self.sort_method, self.reverse_order);
        sort(&mut self.visible_books, self.sort_method, self.reverse_order);

        if update {
            self.update_shelf(false, hub, rq, context);
            let search_visible = rlocate::<SearchBar>(self).is_some();
            self.update_top_bar(search_visible, rq);
            self.update_bottom_bar(rq);
        }
    }

    fn flush(&mut self, context: &mut Context) {
        context.library.flush();
    }

    fn reseed(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
    //     context.library.sort(self.sort_method, self.reverse_order);
    //     self.refresh_visibles(true, false, hub, &mut RenderQueue::new(), context);

        if let Some(top_bar) = self.child_mut(0).downcast_mut::<TopBar>() {
            top_bar.update_frontlight_icon(&mut RenderQueue::new(), context);
            hub.send(Event::ClockTick).ok();
            hub.send(Event::BatteryTick).ok();
        }

        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for Works {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) => {
                match dir {
                    _ => (),
                }
                true
            },
            Event::Gesture(GestureEvent::Rotate { quarter_turns, .. }) if quarter_turns != 0 => {
                let (_, dir) = CURRENT_DEVICE.mirroring_scheme();
                let n = (4 + (context.display.rotation - dir * quarter_turns)) % 4;
                hub.send(Event::Select(EntryId::Rotate(n))).ok();
                true
            },
            Event::Gesture(GestureEvent::Arrow { dir, .. }) => {
                match dir {
                    Dir::West => self.go_to_page(0, hub, rq, context),
                    Dir::East => {
                        let pages_count = self.pages_count;
                        self.go_to_page(pages_count.saturating_sub(1), hub, rq, context);
                    },
                    Dir::North => {
//TODO - add new gesture?
                    },
                    Dir::South => self.toggle_search_bar(None, true, hub, rq, context),
                };
                true
            },
            Event::Focus(v) => {
                if self.focus != v {
                    self.focus = v;
                    if v.is_some() {
                        self.toggle_keyboard(true, true, v, hub, rq, context);
                    }
                }
                true
            },
            Event::Show(ViewId::Keyboard) => {
                self.toggle_keyboard(true, true, None, hub, rq, context);
                true
            },
            Event::Toggle(ViewId::GoToPage) => {
                self.toggle_go_to_page(None, hub, rq, context);
                true
            },
            Event::Toggle(ViewId::SearchBar) => {
                self.toggle_search_bar(None, true, hub, rq, context);
                true
            },
            Event::ToggleNear(ViewId::TitleMenu, rect) => {
                self.toggle_sort_menu(rect, None, rq, context);
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
            Event::Close(ViewId::SearchBar) => {
                self.toggle_search_bar(Some(false), true, hub, rq, context);
                true
            },
            Event::Close(ViewId::SortMenu) => {
                self.toggle_sort_menu(Rectangle::default(), Some(false), rq, context);
                true
            },
            Event::Close(ViewId::MainMenu) => {
                toggle_main_menu(self, Rectangle::default(), Some(false), rq, context);
                true
            },
            Event::Close(ViewId::GoToPage) => {
                self.toggle_go_to_page(Some(false), hub, rq, context);
                true
            },
            Event::Select(EntryId::Sort(sort_method)) => {
                self.set_sort_method(sort_method, hub, rq, context);
                true
            },
            Event::Select(EntryId::ReverseOrder) => {
                let next_value = !self.reverse_order;
                self.set_reverse_order(next_value, hub, rq, context);
                true
            },
            Event::Select(EntryId::Flush) => {
                self.flush(context);
                true
            },
            Event::Select(EntryId::SetStatus(ref path, status)) => {
                self.set_status(path, status, hub, rq, context);
                true
            },
            Event::Select(EntryId::ThumbnailPreviews) => {
                let selected_library = context.settings.selected_library;
                context.settings.libraries[selected_library].thumbnail_previews = !context.settings.libraries[selected_library].thumbnail_previews;
                self.update_thumbnail_previews(hub, rq, context);
                true
            },
            Event::Submit(ViewId::AddressBarInput, ref addr) => {
                self.toggle_keyboard(false, true, None, hub, rq, context);
                true
            },
            Event::Submit(ViewId::HomeSearchInput, ref text) => {
                self.query = BookQuery::new(text);
                if self.query.is_some() {
                    self.toggle_keyboard(false, false, None, hub, rq, context);
                    // Render the search bar and its separator.
                    for i in self.shelf_index + 1 ..= self.shelf_index + 2 {
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                    }
                    self.refresh_visibles(true, true, hub, rq, context);
                } else {
                    let notif = Notification::new(ViewId::InvalidSearchQueryNotif,
                                                  "Invalid search query.".to_string(),
                                                  hub,
                                                  rq,
                                                  context);
                    self.children.push(Box::new(notif) as Box<dyn View>);
                }
                true
            },
            Event::Submit(ViewId::GoToPageInput, ref text) => {
                if text == "(" {
                    self.go_to_page(0, hub, rq, context);
                } else if text == ")" {
                    self.go_to_page(self.pages_count.saturating_sub(1), hub, rq, context);
                } else if text == "_" {
                    let index = (context.rng.next_u64() % self.pages_count as u64) as usize;
                    self.go_to_page(index, hub, rq, context);
                } else if let Ok(index) = text.parse::<usize>() {
                    self.go_to_page(index.saturating_sub(1), hub, rq, context);
                }
                true
            },
            Event::Select(EntryId::Remove(ref path)) => {
                self.remove(path, hub, rq, context)
                    .map_err(|e| eprintln!("{}", e))
                    .ok();
                true
            },
            Event::Select(EntryId::ToggleShowHidden) => {
                context.library.show_hidden = !context.library.show_hidden;
                self.refresh_visibles(true, false, hub, rq, context);
                true
            },
            Event::Select(EntryId::SearchAuthor(ref author)) => {
                let text = format!("'a {}", author);
                let query = BookQuery::new(&text);
                if query.is_some() {
                    self.query = query;
                    self.toggle_search_bar(Some(true), false, hub, rq, context);
                    self.toggle_keyboard(false, false, None, hub, rq, context);
                    if let Some(search_bar) = self.children[self.shelf_index+2].downcast_mut::<SearchBar>() {
                        search_bar.set_text(&text, rq, context);
                    }
                    // Render the search bar and its separator.
                    for i in self.shelf_index + 1 ..= self.shelf_index + 2 {
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                    }
                    self.refresh_visibles(true, true, hub, rq, context);
                }
                true
            },
            Event::GoToTag(ref loc) => {
                println!("trying to go to location {:?}", loc);

                let shelf_index = self.shelf_index.clone();
                let prev_workindex = self.child(shelf_index).downcast_ref::<WorkIndex>().unwrap();
                let rect = prev_workindex.rect().clone();

                let mut workindex = WorkIndex::new(rect,
                            false,
                            loc.clone(),
                            hub,
                            context);


                let current_page = workindex.current_page;
                let pages_count = workindex.max_page;

                self.pages_count = pages_count;
                self.current_page = current_page;
                self.works_count = workindex.max_works;
                self.works_lines = workindex.max_lines;

                workindex.get_works(context, rq);

                self.children_mut().push(Box::new(workindex) as Box<dyn View>);
                self.children_mut().swap_remove(shelf_index);
                self.update_bottom_bar(rq);
                false
            },
            Event::GoTo(location) => {
                self.go_to_page(location as usize, hub, rq, context);
                true
            },
            Event::Chapter(dir) => {
                let pages_count = self.pages_count;
                match dir {
                    CycleDir::Previous => self.go_to_page(0, hub, rq, context),
                    CycleDir::Next => self.go_to_page(pages_count.saturating_sub(1), hub, rq, context),
                }
                true
            },
            Event::Page(dir) => {
                self.go_to_neighbor(dir, hub, rq, context);
                true
            },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Backward, status: ButtonStatus::Pressed, .. }) => {
                self.go_to_neighbor(CycleDir::Previous, hub, rq, context);
                true
            },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Forward, status: ButtonStatus::Pressed, .. }) => {
                self.go_to_neighbor(CycleDir::Next, hub, rq, context);
                true
            },
            Event::Device(DeviceEvent::NetUp) => {
                //TODO new event
                true
            },
            Event::ToggleFrontlight => {
                if let Some(index) = locate::<TopBar>(self) {
                    self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                        .update_frontlight_icon(rq, context);
                }
                true
            },
            Event::Reseed => {
                self.reseed(hub, rq, context);
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);

        self.children.retain(|child| !child.is::<Menu>());

        // Top bar.
        let top_bar_rect = rect![rect.min.x, rect.min.y,
                                 rect.max.x, rect.min.y + small_height - small_thickness];
        self.children[0].resize(top_bar_rect, hub, rq, context);

        let separator_rect = rect![rect.min.x, rect.min.y + small_height - small_thickness,
                                   rect.max.x, rect.min.y + small_height + big_thickness];
        self.children[1].resize(separator_rect, hub, rq, context);

        let mut shelf_min_y = rect.min.y + small_height + big_thickness;
        let mut index = 2;

        // // Navigation bar.
        // if context.settings.home.navigation_bar {
        //     let count = if self.children[self.shelf_index+2].is::<SearchBar>() { 2 } else { 1 };
        //     let nav_bar = self.children[index].as_mut().downcast_mut::<NavigationBar>().unwrap();
        //     let (_, dirs) = context.library.list(&self.current_directory, None, true);
        //     nav_bar.clear();
        //     nav_bar.resize(rect![rect.min.x, shelf_min_y,
        //                          rect.max.x, shelf_min_y + small_height - thickness],
        //                    hub, rq, context);
        //     nav_bar.vertical_limit = rect.max.y - count * small_height - big_height - small_thickness;
        //     nav_bar.set_path(&self.current_directory, &dirs, &mut RenderQueue::new(), context);
        //     shelf_min_y += nav_bar.rect().height() as i32;
        //     index += 1;

        //     self.children[index].resize(rect![rect.min.x, shelf_min_y,
        //                                       rect.max.x, shelf_min_y + thickness],
        //                                 hub, rq, context);
        //     shelf_min_y += thickness;
        // }

        // Bottom bar.
        let bottom_bar_index = rlocate::<BottomBar>(self).unwrap();
        index = bottom_bar_index;

        let separator_rect = rect![rect.min.x, rect.max.y - small_height - small_thickness,
                                   rect.max.x, rect.max.y - small_height + big_thickness];
        self.children[index-1].resize(separator_rect, hub, rq, context);

        let bottom_bar_rect = rect![rect.min.x, rect.max.y - small_height + big_thickness,
                                    rect.max.x, rect.max.y];
        self.children[index].resize(bottom_bar_rect, hub, rq, context);

        let mut shelf_max_y = rect.max.y - small_height - small_thickness;

        if index - self.shelf_index > 2 {
            index -= 2;
            // Keyboard.
            if self.children[index].is::<Keyboard>() {
                let kb_rect = rect![rect.min.x,
                                    rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    rect.max.x,
                                    rect.max.y - small_height - small_thickness];
                self.children[index].resize(kb_rect, hub, rq, context);
                let s_max_y = self.children[index].rect().min.y;
                self.children[index-1].resize(rect![rect.min.x, s_max_y - thickness,
                                                    rect.max.x, s_max_y],
                                              hub, rq, context);
                index -= 2;
            }
            // Search bar.
            if self.children[index].is::<SearchBar>() {
                let sp_rect = *self.children[index+1].rect() - pt!(0, small_height);
                self.children[index].resize(rect![rect.min.x,
                                                  sp_rect.max.y,
                                                  rect.max.x,
                                                  sp_rect.max.y + small_height - thickness],
                                            hub, rq, context);
                self.children[index-1].resize(sp_rect, hub, rq, context);
                shelf_max_y -= small_height;
            }
        }

        // Shelf.
        let shelf_rect = rect![rect.min.x, shelf_min_y,
                               rect.max.x, shelf_max_y];
        self.children[self.shelf_index].resize(shelf_rect, hub, rq, context);

        self.update_shelf(true, hub, &mut RenderQueue::new(), context);
        self.update_bottom_bar(&mut RenderQueue::new());

        // Floating windows.
        for i in bottom_bar_index+1..self.children.len() {
            self.children[i].resize(rect, hub, rq, context);
        }

        self.rect = rect;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
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
}
