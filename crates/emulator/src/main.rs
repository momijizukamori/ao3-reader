use std::mem;
use std::thread;
use std::fs::File;
use std::sync::mpsc;
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;
use ao3reader_core::anyhow::{Error, Context as ResultExt};
use ao3reader_core::chrono::Local;
use ao3reader_core::helpers::get_url;
use ao3reader_core::view::overlay::about::About;
use ao3reader_core::view::works::HistoryView;
use ao3reader_core::view::works::IndexType;
use ao3reader_core::view::works::Works;
use sdl2::event::Event as SdlEvent;
use sdl2::keyboard::{Scancode, Keycode, Mod};
use sdl2::render::{WindowCanvas, BlendMode};
use sdl2::pixels::{Color as SdlColor, PixelFormatEnum};
use sdl2::mouse::MouseState;
use sdl2::rect::Point as SdlPoint;
use sdl2::rect::Rect as SdlRect;
use ao3reader_core::framebuffer::{Framebuffer, UpdateMode};
use ao3reader_core::input::{DeviceEvent, FingerStatus, ButtonCode, ButtonStatus};
use ao3reader_core::document::sys_info_as_html;
use ao3reader_core::view::{View, Event, ViewId, EntryId, AppCmd, EntryKind};
use ao3reader_core::view::{process_render_queue, wait_for_all, handle_event, RenderQueue, RenderData};
use ao3reader_core::view::home::Home;
use ao3reader_core::view::reader::Reader;
use ao3reader_core::view::notification::Notification;
use ao3reader_core::view::dialog::Dialog;
use ao3reader_core::view::frontlight::FrontlightWindow;
use ao3reader_core::view::menu::{Menu, MenuKind};
use ao3reader_core::view::intermission::Intermission;
use ao3reader_core::view::dictionary::Dictionary;
use ao3reader_core::view::touch_events::TouchEvents;
use ao3reader_core::view::rotation_values::RotationValues;
use ao3reader_core::view::common::{locate, locate_by_id, transfer_notifications, overlapping_rectangle};
use ao3reader_core::view::common::{toggle_input_history_menu, toggle_keyboard_layout_menu};
use ao3reader_core::helpers::{save_toml};
use ao3reader_core::settings::{SETTINGS_PATH, IntermKind};
use ao3reader_core::geom::{Rectangle, Axis};
use ao3reader_core::gesture::{GestureEvent, gesture_events};
use ao3reader_core::device::CURRENT_DEVICE;
use ao3reader_core::context::Context;
use ao3reader_core::pt;
use ao3reader_core::png;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::StatusCode;

pub const APP_NAME: &str = "AO3 Reader";
const DEFAULT_ROTATION: i8 = 1;

const CLOCK_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[inline]
fn seconds(timestamp: u32) -> f64 {
    timestamp as f64 / 1000.0
}

#[inline]
pub fn device_event(event: SdlEvent) -> Option<DeviceEvent> {
    match event {
        SdlEvent::MouseButtonDown { timestamp, x, y, .. } =>
            Some(DeviceEvent::Finger { id: 0,
                                       status: FingerStatus::Down,
                                       position: pt!(x, y),
                                       time: seconds(timestamp) }),
        SdlEvent::MouseButtonUp { timestamp, x, y, .. } =>
            Some(DeviceEvent::Finger { id: 0,
                                       status: FingerStatus::Up,
                                       position: pt!(x, y),
                                       time: seconds(timestamp) }),
        SdlEvent::MouseMotion { timestamp, x, y, .. } =>
            Some(DeviceEvent::Finger { id: 0,
                                       status: FingerStatus::Motion,
                                       position: pt!(x, y),
                                       time: seconds(timestamp) }),
        _ => None,
    }
}

fn code_from_key(key: Scancode) -> Option<ButtonCode> {
    match key {
        Scancode::B => Some(ButtonCode::Backward),
        Scancode::F => Some(ButtonCode::Forward),
        Scancode::P => Some(ButtonCode::Power),
        Scancode::L => Some(ButtonCode::Light),
        Scancode::H => Some(ButtonCode::Home),
        Scancode::E => Some(ButtonCode::Erase),
        Scancode::G => Some(ButtonCode::Highlight),
        _ => None,
    }
}

struct FBCanvas(WindowCanvas);

impl Framebuffer for FBCanvas {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        self.0.set_draw_color(SdlColor::RGB(color, color, color));
        self.0.draw_point(SdlPoint::new(x as i32, y as i32)).unwrap();
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        self.0.set_draw_color(SdlColor::RGBA(color, color, color, (alpha * 255.0) as u8));
        self.0.draw_point(SdlPoint::new(x as i32, y as i32)).unwrap();
    }

    fn invert_region(&mut self, rect: &Rectangle) {
        let width = rect.width();
        let s_rect = Some(SdlRect::new(rect.min.x, rect.min.y,
                                       width, rect.height()));
        if let Ok(data) = self.0.read_pixels(s_rect, PixelFormatEnum::RGB24) {
            for y in rect.min.y..rect.max.y {
                let v = (y - rect.min.y) as u32;
                for x in rect.min.x..rect.max.x {
                    let u = (x - rect.min.x) as u32;
                    let addr = 3 * (v * width + u);
                    let color = 255 - data[addr as usize];
                    self.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }

    fn shift_region(&mut self, rect: &Rectangle, drift: u8) {
        let width = rect.width();
        let s_rect = Some(SdlRect::new(rect.min.x, rect.min.y,
                                       width, rect.height()));
        if let Ok(data) = self.0.read_pixels(s_rect, PixelFormatEnum::RGB24) {
            for y in rect.min.y..rect.max.y {
                let v = (y - rect.min.y) as u32;
                for x in rect.min.x..rect.max.x {
                    let u = (x - rect.min.x) as u32;
                    let addr = 3 * (v * width + u);
                    let color = data[addr as usize].saturating_sub(drift);
                    self.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }

    fn update(&mut self, _rect: &Rectangle, _mode: UpdateMode) -> Result<u32, Error> {
        self.0.present();
        Ok(Local::now().timestamp_subsec_millis())
    }

    fn wait(&self, _tok: u32) -> Result<i32, Error> {
        Ok(1)
    }

    fn save(&self, path: &str) -> Result<(), Error> {
        let (width, height) = self.dims();
        let file = File::create(path).with_context(|| format!("can't create output file {}", path))?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().with_context(|| format!("can't write PNG header for {}", path))?;
        let data = self.0.read_pixels(self.0.viewport(), PixelFormatEnum::RGB24).unwrap_or_default();
        writer.write_image_data(&data).with_context(|| format!("can't write PNG data to {}", path))?;
        Ok(())
    }

    fn rotation(&self) -> i8 {
        DEFAULT_ROTATION
    }

    fn set_rotation(&mut self, n: i8) -> Result<(u32, u32), Error> {
        let (mut width, mut height) = self.dims();
        if (width < height && n % 2 == 0) || (width > height && n % 2 == 1) {
            mem::swap(&mut width, &mut height);
        }
        self.0.window_mut().set_size(width, height).ok();
        Ok((width, height))
    }

    fn set_monochrome(&mut self, _enable: bool) {
    }

    fn set_dithered(&mut self, _enable: bool) {
    }

    fn set_inverted(&mut self, _enable: bool) {
    }

    fn monochrome(&self) -> bool {
        false
    }

    fn dithered(&self) -> bool {
        false
    }

    fn inverted(&self) -> bool {
        false
    }

    fn width(&self) -> u32 {
        self.0.window().size().0
    }

    fn height(&self) -> u32 {
        self.0.window().size().1
    }
}

fn main() -> Result<(), Error> {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let (width, height) = CURRENT_DEVICE.dims;
    let window = video_subsystem
                 .window("AO3 Reader Emulator", width, height)
                 .position_centered()
                 .build()
                 .unwrap();

    let mut fb = window.into_canvas().software().build().unwrap();
    fb.set_blend_mode(BlendMode::Blend);

    let mut context = Context::new_from_virtual(Box::new(FBCanvas(fb)));
    context.client.renew_login();

    if context.settings.import.startup_trigger {
        context.batch_import();
    }

    context.load_dictionaries();
    context.load_keyboard_layouts();

    // Add input sources into a single FIFO queue
    let (tx, rx) = mpsc::channel();
    let (ty, ry) = mpsc::channel();
    let touch_screen = gesture_events(ry);

    let tx2 = tx.clone();
    thread::spawn(move || {
        while let Ok(evt) = touch_screen.recv() {
            tx2.send(evt).ok();
        }
    });

    let tx3 = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(CLOCK_REFRESH_INTERVAL);
            tx3.send(Event::ClockTick).ok();
        }
    });

    let mut history: Vec<Box<dyn View>> = Vec::new();
    let mut rq = RenderQueue::new();
    let mut view: Box<dyn View> = Box::new(Home::new(context.fb.rect(), &mut rq,
            context.settings.time_format.clone(), &mut context.fonts, &mut context.battery, context.settings.frontlight, context.client.logged_in, &context.settings.ao3.faves));

    let mut updating = Vec::new();

    if context.settings.frontlight {
        let levels = context.settings.frontlight_levels;
        context.frontlight.set_intensity(levels.intensity);
        context.frontlight.set_warmth(levels.warmth);
    } else {
        context.frontlight.set_warmth(0.0);
        context.frontlight.set_intensity(0.0);
    }

    println!("{} is running virtually {}.", APP_NAME,
                                            CURRENT_DEVICE.model);
    println!("The framebuffer resolution is {} by {}.", context.fb.rect().width(),
                                                        context.fb.rect().height());

    let mut bus = VecDeque::with_capacity(4);

    // Handle the inputs
    // TODO - why are these different between the Kobo app and the emulator
    'outer: loop {
        let mut event_pump = sdl_context.event_pump().unwrap();
        while let Some(sdl_evt) = event_pump.poll_event() {
            match sdl_evt {
                SdlEvent::Quit { .. } |
                SdlEvent::KeyDown { keycode: Some(Keycode::Escape), keymod: Mod::NOMOD, .. } => {
                    view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), &mut RenderQueue::new(), &mut context);
                    while let Some(mut view) = history.pop() {
                        view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), &mut RenderQueue::new(), &mut context);
                    }
                    break 'outer;
                },
                SdlEvent::KeyUp { scancode: Some(scancode), keymod: Mod::NOMOD, timestamp, .. } => {
                    if let Some(code) = code_from_key(scancode) {
                        ty.send(DeviceEvent::Button {
                            time: seconds(timestamp),
                            code,
                            status: ButtonStatus::Released,
                        }).ok();
                    }
                },
                SdlEvent::KeyDown { scancode: Some(scancode), keymod, timestamp, repeat, .. } => {
                    match keymod {
                        Mod::NOMOD => {
                            match scancode {
                                Scancode::LeftBracket => {
                                    let rot = (3 + context.display.rotation) % 4;
                                    ty.send(DeviceEvent::RotateScreen(rot)).ok();
                                },
                                Scancode::RightBracket => {
                                    let rot = (5 + context.display.rotation) % 4;
                                    ty.send(DeviceEvent::RotateScreen(rot)).ok();
                                },
                                Scancode::S => {
                                    tx.send(Event::Select(EntryId::TakeScreenshot)).ok();
                                },
                                Scancode::B | Scancode::F | Scancode::P | Scancode::L | Scancode::H |
                                    Scancode::E | Scancode::G => {
                                    if let Some(code) = code_from_key(scancode) {
                                        let status = if repeat {
                                            ButtonStatus::Repeated
                                        } else {
                                            ButtonStatus::Pressed
                                        };
                                        ty.send(DeviceEvent::Button {
                                            time: seconds(timestamp),
                                            code,
                                            status,
                                        }).ok();
                                    }
                                },
                                Scancode::I | Scancode::O => {
                                    let mouse_state = MouseState::new(&event_pump);
                                    let x = mouse_state.x() as i32;
                                    let y = mouse_state.y() as i32;
                                    let center = pt!(x, y);
                                    if scancode == Scancode::I {
                                        tx.send(Event::Gesture(GestureEvent::Spread { center,
                                                                                      factor: 2.0,
                                                                                      axis: Axis::Diagonal })).ok();
                                    } else {
                                        tx.send(Event::Gesture(GestureEvent::Pinch { center,
                                                                                     factor: 0.5,
                                                                                     axis: Axis::Diagonal })).ok();
                                    }
                                },
                                _ => (),
                            }
                        },
                        Mod::LSHIFTMOD | Mod::RSHIFTMOD => {
                            match scancode {
                                Scancode::S | Scancode::P | Scancode::C => {
                                    if let Some(index) = locate::<Intermission>(view.as_ref()) {
                                        let rect = *view.child(index).rect();
                                        view.children_mut().remove(index);
                                        rq.add(RenderData::expose(rect, UpdateMode::Full));
                                    } else {
                                        view.handle_event(&Event::Suspend, &tx, &mut VecDeque::new(), &mut RenderQueue::new(), &mut context);
                                        let kind = match scancode {
                                            Scancode::S => IntermKind::Suspend,
                                            Scancode::P => IntermKind::PowerOff,
                                            Scancode::C => IntermKind::Share,
                                            _ => unreachable!(),
                                        };
                                        let interm = Intermission::new(context.fb.rect(), kind, &context);
                                        rq.add(RenderData::new(interm.id(), *interm.rect(), UpdateMode::Full));
                                        view.children_mut().push(Box::new(interm) as Box<dyn View>);
                                    }
                                },
                                _ => (),
                            }
                        },
                        _ => (),
                    }
                },
                _ => {
                    if let Some(dev_evt) = device_event(sdl_evt) {
                        ty.send(dev_evt).ok();
                    }
                },
            }
        }

        while let Ok(evt) = rx.recv_timeout(Duration::from_millis(20)) {
            match evt {
                Event::Open(info) => {
                    let rotation = context.display.rotation;
                    if let Some(n) = info.reader.as_ref()
                                         .and_then(|r| r.rotation.map(|n| CURRENT_DEVICE.from_canonical(n))) {
                        if n != rotation {
                            if let Ok(dims) = context.fb.set_rotation(n) {
                                context.display.rotation = n;
                                context.display.dims = dims;
                            }
                        }
                    }
                    let path = info.file.path.clone();
                    if let Some(r) = Reader::new(context.fb.rect(), *info, &tx, &mut context) {
                        let mut next_view = Box::new(r) as Box<dyn View>;
                        transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                        history.push(view as Box<dyn View>);
                        view = next_view;
                    } else {
                        if context.display.rotation != rotation {
                            if let Ok(dims) = context.fb.set_rotation(rotation) {
                                context.display.rotation = rotation;
                                context.display.dims = dims;
                            }
                        }
                        handle_event(view.as_mut(), &Event::Invalid(path), &tx, &mut bus, &mut rq, &mut context);
                    }
                },
                Event::OpenWork(id) => {
                    let uri = format!("https://archiveofourown.org/works/{}?view_full_work=true&view_adult=true", id);
                    let html = context.client.get_html(&uri);
                    let r = Reader::from_ao3(context.fb.rect(), &html, Some(&uri), &tx, &mut context);
                    let mut next_view = Box::new(r) as Box<dyn View>;
                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                    history.push(view as Box<dyn View>);
                    view = next_view;
    
                },
                Event::OpenHtml(ref html, ref link_uri) => {
                    view.children_mut().retain(|child| !child.is::<Menu>());
                    let r = Reader::from_ao3(context.fb.rect(), html, link_uri.as_deref(), &tx, &mut context);
                    let mut next_view = Box::new(r) as Box<dyn View>;
                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                    history.push(view as Box<dyn View>);
                    view = next_view;
                },
                Event::LoadIndex( link_uri) => {
                    println!("loading tag {}", link_uri);
    
                    let url = get_url(&link_uri);
                    let client = Client::builder().redirect(Policy::none()).build().unwrap();
                    let res = client.get(url.as_str()).send();
                    match res {
                        Ok(r) => {
                            match r.status() {
                                StatusCode::OK => {
                                    view.children_mut().retain(|child| !child.is::<Menu>());
                                    let mut next_view: Box<dyn View> = Box::new(Works::new(context.fb.rect(), link_uri, &tx,
                                                                         &mut rq, &mut context, ao3reader_core::view::works::IndexType::TagWorks)?);
                                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                                    history.push(view as Box<dyn View>);
                                    view = next_view;
                                },
                                StatusCode::FOUND |
                                StatusCode::MOVED_PERMANENTLY => {
                                    // Check if we're being redirected to a different works index
                                    // because of tag synning
                                    if let Some(loc) = r.headers().get(reqwest::header::LOCATION) {
                                        if let Ok(loc) = loc.to_str() {
                                            let loc_str = loc.to_string();
                                            if loc_str.ends_with("/works") {
                                                view.children_mut().retain(|child| !child.is::<Menu>());
                                                let mut next_view: Box<dyn View> = Box::new(Works::new(context.fb.rect(), loc_str, &tx,
                                                                                     &mut rq, &mut context, ao3reader_core::view::works::IndexType::TagWorks)?);
                                                transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                                                history.push(view as Box<dyn View>);
                                                view = next_view;
                                            }
                                        }
                                    } else {
                                        // TODO: change this when we can look at tag pages
                                        let msg = format!("Unwrangled tag! No works available.");
                                        let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                                        view.children_mut().push(Box::new(notif) as Box<dyn View>);
                                    }
                                },
                                _ => {
                                    println!("Got {} for {}", r.status(), link_uri);
                                    let msg = format!("Error: {}", r.status());
                                    let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                                    view.children_mut().push(Box::new(notif) as Box<dyn View>);
                                }
                            }
                        },
                        Err(e) => {
                            println!("Error fetching {} - {}", link_uri, e);
                            let msg = format!("Error: {}", e);
                            let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                            view.children_mut().push(Box::new(notif) as Box<dyn View>);
                        }
                    };
    
                },
                Event::LoadSearch( query) => {
                    println!("loading search {}", query);
    
                    let link_uri = format!("https://archiveofourown.org/works/search?work_search%5Bquery%5D={}", query);
                    let url = get_url(&link_uri);
                    let client = Client::builder().redirect(Policy::none()).build().unwrap();
                    let res = client.get(url.as_str()).send();
                    match res {
                        Ok(r) => {
                            match r.status() {
                                StatusCode::OK => {
                                    view.children_mut().retain(|child| !child.is::<Menu>());
                                    let mut next_view: Box<dyn View> = Box::new(Works::new(context.fb.rect(), link_uri, &tx,
                                                                         &mut rq, &mut context, IndexType::Search(query))?);
                                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                                    history.push(view as Box<dyn View>);
                                    view = next_view;
                                },
                                StatusCode::FOUND |
                                    StatusCode::MOVED_PERMANENTLY => {
                                        // Check if we're being redirected to a different works index
                                        // because of tag synning
                                        if let Some(loc) = r.headers().get(reqwest::header::LOCATION) {
                                            if let Ok(loc) = loc.to_str() {
                                                let loc_str = loc.to_string();
                                                if loc_str.ends_with("/works") {
                                                    view.children_mut().retain(|child| !child.is::<Menu>());
                                                    let mut next_view: Box<dyn View> = Box::new(Works::new(context.fb.rect(), loc_str, &tx,
                                                                                         &mut rq, &mut context, ao3reader_core::view::works::IndexType::TagWorks)?);
                                                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                                                    history.push(view as Box<dyn View>);
                                                    view = next_view;
                                                }
                                            }
                                        } else {
                                            // TODO: change this when we can look at tag pages
                                            let msg = format!("Unwrangled tag! No works available.");
                                            let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                                            view.children_mut().push(Box::new(notif) as Box<dyn View>);
                                        }
                                },
                                _ => {
                                    println!("Got {} for {}", r.status(), link_uri);
                                    let msg = format!("Error: {}", r.status());
                                    let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                                    view.children_mut().push(Box::new(notif) as Box<dyn View>);
                                }
                            }
                        },
                        Err(e) => {
                            println!("Error fetching {} - {}", link_uri, e);
                            let msg = format!("Error: {}", e);
                            let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                            view.children_mut().push(Box::new(notif) as Box<dyn View>);
                        }
                    };
    
                },
                Event::LoadHistory(history_view) => {
                    if let Some(ref username) = context.settings.ao3.username {
    
                        let mut link_uri = format!("https://archiveofourown.org/users/{}/readings", username);
                        if let HistoryView::MarkedForLater = history_view {
                             link_uri = link_uri + "?show=to-read";
                        }
    
                        let res = context.client.get(&link_uri).send();
                        match res {
                            Ok(r) => {
                                match r.status() {
                                    StatusCode::OK => {
                                        view.children_mut().retain(|child| !child.is::<Menu>());
                                        let mut next_view: Box<dyn View> = Box::new(Works::new(context.fb.rect(), link_uri, &tx,
                                                                             &mut rq, &mut context, IndexType::History(history_view))?);
                                        transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                                        history.push(view as Box<dyn View>);
                                        view = next_view;
                                    },
                                    _ => {
                                        println!("Got {} for {}", r.status(), link_uri);
                                        let msg = format!("Error: {}", r.status());
                                        let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                                        view.children_mut().push(Box::new(notif) as Box<dyn View>);
                                    }
                                }
                            },
                            Err(e) => {
                                println!("Error fetching {} - {}", link_uri, e);
                                let msg = format!("Error: {}", e);
                                let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                                view.children_mut().push(Box::new(notif) as Box<dyn View>);
                            }
                        };
                    } else {
                        println!("Can't load history without a username!");
                        let msg = format!("Can't load history without a username!");
                        let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                        view.children_mut().push(Box::new(notif) as Box<dyn View>);
                    }
                },
                Event::Select(EntryId::Launch(app_cmd)) => {
                    view.children_mut().retain(|child| !child.is::<Menu>());
                    let mut next_view: Box<dyn View> = match app_cmd {
                        AppCmd::Dictionary { ref query, ref language } => {
                            Box::new(Dictionary::new(context.fb.rect(), query, language, &tx, &mut rq, &mut context))
                        },
                        AppCmd::TouchEvents => {
                            Box::new(TouchEvents::new(context.fb.rect(), &mut rq, &mut context))
                        },
                        AppCmd::RotationValues => {
                            Box::new(RotationValues::new(context.fb.rect(), &mut rq, &mut context))
                        },
                    };
                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                    history.push(view as Box<dyn View>);
                    view = next_view;
                },
                Event::Back => {
                    if let Some(v) = history.pop() {
                        view = v;
                        if view.is::<Home>() {
                            if context.display.rotation % 2 != 1 {
                                if let Ok(dims) = context.fb.set_rotation(DEFAULT_ROTATION) {
                                    context.display.rotation = DEFAULT_ROTATION;
                                    context.display.dims = dims;
                                }
                            }
                        }
                        view.handle_event(&Event::Reseed, &tx, &mut bus, &mut rq, &mut context);
                    }
                },
                Event::TogglePresetMenu(rect, index) => {
                    if let Some(index) = locate_by_id(view.as_ref(), ViewId::PresetMenu) {
                        let rect = *view.child(index).rect();
                        view.children_mut().remove(index);
                        rq.add(RenderData::expose(rect, UpdateMode::Gui));
                    } else {
                        let preset_menu = Menu::new(rect, ViewId::PresetMenu, MenuKind::Contextual,
                                                    vec![EntryKind::Command("Remove".to_string(),
                                                                            EntryId::RemovePreset(index))],
                                                    &mut context);
                        rq.add(RenderData::new(preset_menu.id(), *preset_menu.rect(), UpdateMode::Gui));
                        view.children_mut().push(Box::new(preset_menu) as Box<dyn View>);
                    }
                },
                Event::ToggleAboutWork(info) => {
                    println!("Trying to open about work overlay");
                    let mut about_overlay = About::new(info, &mut context);
                    about_overlay.update_page();
                    rq.add(RenderData::new(about_overlay.id(), *about_overlay.rect(), UpdateMode::Gui));
                    view.children_mut().push(Box::new(about_overlay) as Box<dyn View>);
                 },
                 Event::ToggleFave(title, url)  => {
                    context.settings.ao3.toggle_fave(title, url);
                 },
                Event::Show(ViewId::Frontlight) => {
                    if !context.settings.frontlight {
                        context.set_frontlight(true);
                        view.handle_event(&Event::ToggleFrontlight, &tx, &mut bus, &mut rq, &mut context);
                    }
                    let flw = FrontlightWindow::new(&mut context);
                    rq.add(RenderData::new(flw.id(), *flw.rect(), UpdateMode::Gui));
                    view.children_mut().push(Box::new(flw) as Box<dyn View>);
                },
                Event::ToggleFrontlight => {
                    context.set_frontlight(!context.settings.frontlight);
                    view.handle_event(&Event::ToggleFrontlight, &tx, &mut bus, &mut rq, &mut context);
                },
                Event::ToggleInputHistoryMenu(id, rect) => {
                    toggle_input_history_menu(view.as_mut(), id, rect, None, &mut rq, &mut context);
                },
                Event::ToggleNear(ViewId::KeyboardLayoutMenu, rect) => {
                    toggle_keyboard_layout_menu(view.as_mut(), rect, None, &mut rq, &mut context);
                },
                Event::Close(ViewId::Frontlight) => {
                    if let Some(index) = locate::<FrontlightWindow>(view.as_ref()) {
                        let rect = *view.child(index).rect();
                        view.children_mut().remove(index);
                        rq.add(RenderData::expose(rect, UpdateMode::Gui));
                    }
                },
                Event::Close(id) => {
                    if let Some(index) = locate_by_id(view.as_ref(), id) {
                        let rect = overlapping_rectangle(view.child(index));
                        rq.add(RenderData::expose(rect, UpdateMode::Gui));
                        view.children_mut().remove(index);
                    }
                },
                Event::Select(EntryId::About) => {
                    let dialog = Dialog::new(ViewId::AboutDialog,
                                             None,
                                             format!("Plato {}", env!("CARGO_PKG_VERSION")),
                                             &mut context);
                    rq.add(RenderData::new(dialog.id(), *dialog.rect(), UpdateMode::Gui));
                    view.children_mut().push(Box::new(dialog) as Box<dyn View>);
                },
                Event::Select(EntryId::SystemInfo) => {
                    view.children_mut().retain(|child| !child.is::<Menu>());
                    let html = sys_info_as_html();
                    let r = Reader::from_html(context.fb.rect(), &html, None, &tx, &mut context);
                    let mut next_view = Box::new(r) as Box<dyn View>;
                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut rq, &mut context);
                    history.push(view as Box<dyn View>);
                    view = next_view;
                },
                Event::Select(EntryId::Rotate(n)) if n != context.display.rotation && view.might_rotate() => {
                    wait_for_all(&mut updating, &mut context);
                    if let Ok(dims) = context.fb.set_rotation(n) {
                        context.display.rotation = n;
                        let fb_rect = Rectangle::from(dims);
                        if context.display.dims != dims {
                            context.display.dims = dims;
                            view.resize(fb_rect, &tx, &mut rq, &mut context);
                        }
                    }
                },
                Event::Select(EntryId::SetButtonScheme(button_scheme)) => {
                    context.settings.button_scheme = button_scheme;
                },
                Event::Select(EntryId::ToggleInverted) => {
                    context.fb.toggle_inverted();
                    rq.add(RenderData::new(view.id(), context.fb.rect(), UpdateMode::Gui));
                },
                Event::Select(EntryId::TakeScreenshot) => {
                    let name = Local::now().format("screenshot-%Y%m%d_%H%M%S.png");
                    let msg = match context.fb.save(&name.to_string()) {
                        Err(e) => format!("Couldn't take screenshot: {}).", e),
                        Ok(_) => format!("Saved {}.", name),
                    };
                    let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                    view.children_mut().push(Box::new(notif) as Box<dyn View>);
                },
                Event::Notify(msg) => {
                    let notif = Notification::new(msg, &tx, &mut rq, &mut context);
                    view.children_mut().push(Box::new(notif) as Box<dyn View>);
                },
                Event::Device(DeviceEvent::NetUp) |
                Event::CheckFetcher(..) |
                Event::FetcherAddDocument(..) |
                Event::FetcherRemoveDocument(..) |
                Event::FetcherSearch { .. } if !view.is::<Home>() => {
                    if let Some(home) = history.get_mut(0).filter(|view| view.is::<Home>()) {
                        let (tx, _rx) = mpsc::channel();
                        home.handle_event(&evt, &tx, &mut VecDeque::new(), &mut RenderQueue::new(), &mut context);
                    }
                },
                Event::SetWifi(enable) => {
                    if context.settings.wifi != enable {
                        context.settings.wifi = enable;
                        if enable {
                            let tx2 = tx.clone();
                            thread::spawn(move || {
                                thread::sleep(Duration::from_secs(2));
                                tx2.send(Event::Device(DeviceEvent::NetUp)).ok();
                            });
                        } else {
                            context.online = false;
                        }
                    }
                },
                Event::Device(DeviceEvent::RotateScreen(n)) => {
                    tx.send(Event::Select(EntryId::Rotate(n))).ok();
                },
                Event::Select(EntryId::Quit) => {
                    break 'outer;
                },
                _ => {
                    handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut rq, &mut context);
                },
            }
        }

        process_render_queue(view.as_ref(), &mut rq, &mut context, &mut updating);

        while let Some(ce) = bus.pop_front() {
            tx.send(ce).ok();
        }
    }

    if !history.is_empty() {
        let (tx, _rx) = mpsc::channel();
        view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), &mut RenderQueue::new(), &mut context);
        while let Some(mut view) = history.pop() {
            view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), &mut RenderQueue::new(), &mut context);
        }
    }

    if context.settings.frontlight {
        context.settings.frontlight_levels = context.frontlight.levels();
    }

    context.library.flush();

    let path = Path::new(SETTINGS_PATH);
    save_toml(&context.settings, path).context("can't save settings")?;

    Ok(())
}
