use std::sync::mpsc;

use super::*;
use crate::battery::FakeBattery;

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
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
#[coverage(off)]
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
#[coverage(off)]
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
#[coverage(off)]
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
#[coverage(off)]
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

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn WHEN_openSearchBarIsCalled_THEN_aSearchBarAndAKeyboardAreCreated() {
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    // WHEN open_search_bar() is called
    home.open_search_bar(&keyboard_layouts, "test_keyboard".to_string(), &mut rq);
    // THEN a search bar and keyboard are created
    // Ignore all the normal children before the search bar
    assert_eq!(locate::<SearchBar>(&home).unwrap(), 4);
    assert_eq!(locate::<Keyboard>(&home).unwrap(), 5);
    assert_eq!(rlocate::<BottomBar>(&home).unwrap(), 7);
}

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn GIVEN_anOpenSearchBar_WHEN_closeSearchBarIsCalled_THEN_noSearchBarExists() {
    // GIVEN an open search bar
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    home.open_search_bar(&keyboard_layouts, "test_keyboard".to_string(), &mut rq);
    // WHEN close_search_bar is called
    home.close_search_bar(&mut rq);
    // THEN no search bar exits
    // Ignore all the normal children before the search bar
    assert_eq!(locate::<SearchBar>(&home), None);
    assert_eq!(locate::<Keyboard>(&home), None);
    assert_eq!(rlocate::<BottomBar>(&home).unwrap(), 5);
}

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn WHEN_handleSearchEventsIsCalledWithEventToggleSearchBar_THEN_aSearchBarAndAKeyboardAreCreated() {
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    let (tx, _rx) = mpsc::channel();
    // WHEN handle_search_events is called with Event::Toggle(ViewId::SearchBar)
    home.handle_search_events(&Event::Toggle(ViewId::SearchBar), &keyboard_layouts, "test_keyboard".to_string(), &mut rq, &tx);
    // THEN a search bar and keyboard are created
    // Ignore all the normal children before the search bar
    assert_eq!(locate::<SearchBar>(&home).unwrap(), 4);
    assert_eq!(locate::<Keyboard>(&home).unwrap(), 5);
    assert_eq!(rlocate::<BottomBar>(&home).unwrap(), 7);
}

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn GIVEN_anOpenSearchBar_WHEN_handleSearchEventsIsCalledWithEventToggleSearchBar_THEN_noSearchBarExists() {
    // GIVEN an open search bar
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    home.open_search_bar(&keyboard_layouts, "test_keyboard".to_string(), &mut rq);
    let (tx, _rx) = mpsc::channel();
    // WHEN handle_search_events is called with Event::Toggle(ViewId::SearchBar)
    home.handle_search_events(&Event::Toggle(ViewId::SearchBar), &keyboard_layouts, "test_keyboard".to_string(), &mut rq, &tx);
    // THEN no search bar exits
    // Ignore all the normal children before the search bar
    assert_eq!(locate::<SearchBar>(&home), None);
    assert_eq!(locate::<Keyboard>(&home), None);
    assert_eq!(rlocate::<BottomBar>(&home).unwrap(), 5);
}

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn GIVEN_anOpenSearchBar_WHEN_handleSearchEventsIsCalledWithEventCloseSearchBar_THEN_noSearchBarExists() {
    // GIVEN an open search bar
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    home.open_search_bar(&keyboard_layouts, "test_keyboard".to_string(), &mut rq);
    let (tx, _rx) = mpsc::channel();
    // WHEN handle_search_events is called with Event::Close(ViewId::SearchBar)
    home.handle_search_events(&Event::Close(ViewId::SearchBar), &keyboard_layouts, "test_keyboard".to_string(), &mut rq, &tx);
    // THEN no search bar exits
    // Ignore all the normal children before the search bar
    assert_eq!(locate::<SearchBar>(&home), None);
    assert_eq!(locate::<Keyboard>(&home), None);
    assert_eq!(rlocate::<BottomBar>(&home).unwrap(), 5);
}

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn GIVEN_anOpenSearchBar_WHEN_handleSearchEventsIsCalledWithEventSubmitSiteTextSearchInput_THEN_noSearchBarExists_AND_anEventLoadSearchWasSent() {
    // GIVEN an open search bar
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    home.open_search_bar(&keyboard_layouts, "test_keyboard".to_string(), &mut rq);
    let (tx, rx) = mpsc::channel();
    // WHEN handle_search_events is called with Event::Submit(ViewId::SiteTextSearchInput)
    home.handle_search_events(
        &Event::Submit(ViewId::SiteTextSearchInput, "fake_search".to_string()),
        &keyboard_layouts, "test_keyboard".to_string(), &mut rq, &tx);
    // THEN no search bar exits
    // Ignore all the normal children before the search bar
    assert_eq!(locate::<SearchBar>(&home), None);
    assert_eq!(locate::<Keyboard>(&home), None);
    assert_eq!(rlocate::<BottomBar>(&home).unwrap(), 5);
    // AND an Event::LoadSearch was sent
    match rx.recv() {
        Ok(Event::LoadSearch(search_text)) => assert_eq!(search_text, "fake_search".to_string()),
        Ok(event) => panic!("Recieved {event:?} but expected Event::LoadSearch"),
        _ => panic!("Did not recieve any event")
    }
}

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn GIVEN_anOpenSearchBar_WHEN_handleSearchEventsIsCalledWithEventSubmitSiteTextSearchInput_THEN_anEventSubmitInputWasSent() {
    // GIVEN an open search bar
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    home.open_search_bar(&keyboard_layouts, "test_keyboard".to_string(), &mut rq);
    let (tx, rx) = mpsc::channel();
    // WHEN handle_search_events is called with Event::ToggleNear(ViewId::SearchMenu)
    home.handle_search_events(
        &Event::ToggleNear(ViewId::SearchMenu, rect![0,0,1,1]),
        &keyboard_layouts, "test_keyboard".to_string(), &mut rq, &tx);
    // THEN an Event::SubmitInput(ViewId::SiteTextSearchInput) was sent
    match rx.recv() {
        Ok(Event::SubmitInput(view_id)) => assert_eq!(view_id, ViewId::SiteTextSearchInput),
        Ok(event) => panic!("Recieved {event:?} but expected Event::ToggleNear"),
        _ => panic!("Did not recieve any event")
    }
}

#[test]
#[allow(non_snake_case)]
#[coverage(off)]
fn GIVEN_anOpenSearchBar_WHEN_handleSearchEventsIsCalledWithEventFocus_THEN_focusWillBeOnThatView() {
    // GIVEN an open search bar
    let mut battery = Box::new(FakeBattery::new()) as Box<dyn Battery>;
    let mut rq = RenderQueue::new();
    let mut home = Home::new(rect![0, 0, 600, 800], &mut rq, "%H:%M".to_string(), &mut Fonts::load_with_prefix("../../").unwrap(),
                                &mut battery, true, true, &vec![("Test Fave".to_string(), Url::parse("https://fakeo3.org/tags/super-fake").expect("Test URL"))]);
    let mut keyboard_layouts = BTreeMap::new();
    keyboard_layouts.insert("test_keyboard".to_string(), Layout::default());
    home.open_search_bar(&keyboard_layouts, "test_keyboard".to_string(), &mut rq);
    let (tx, _rx) = mpsc::channel();
    // WHEN handle_search_events is called with Event::Focus(ViewId::SearchMenu)
    home.handle_search_events(
        &Event::Focus(Some(ViewId::SearchMenu)),
        &keyboard_layouts, "test_keyboard".to_string(), &mut rq, &tx);
    // THEN focus will be on that view
    assert_eq!(home.focus, Some(ViewId::SearchMenu));
}
