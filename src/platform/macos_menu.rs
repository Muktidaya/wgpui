use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags, NSMenu,
    NSMenuDelegate, NSMenuItem, NSMenuItemValidation,
};
use objc2_foundation::{
    MainThreadMarker, NSInteger, NSObject, NSObjectProtocol, NSProcessInfo, NSString, ns_string,
};

use crate::{Action, Keymap, Modifiers, OwnedMenu, OwnedMenuItem, SystemMenuType};

thread_local! {
    static MENU_RUNTIME: RefCell<Option<MenuRuntime>> = const { RefCell::new(None) };
}

struct MenuRuntime {
    _main_menu: Retained<NSMenu>,
    _target: Retained<MenuTarget>,
    actions: Vec<Box<dyn Action>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "WGPUIAppMenuTarget"]
    #[ivars = ()]
    struct MenuTarget;

    unsafe impl NSObjectProtocol for MenuTarget {}

    #[allow(non_snake_case)]
    unsafe impl NSMenuItemValidation for MenuTarget {
        #[unsafe(method(validateMenuItem:))]
        fn validateMenuItem(&self, menu_item: &NSMenuItem) -> bool {
            let action_identifier = menu_item.tag();
            super::platform::with_active_platform(|platform| {
                platform.validate_menu_action(action_identifier as usize)
            })
            .unwrap_or(false)
        }
    }

    #[allow(non_snake_case)]
    unsafe impl NSMenuDelegate for MenuTarget {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menuNeedsUpdate(&self, _menu: &NSMenu) {
            super::platform::with_active_platform(|platform| {
                platform.will_open_app_menu();
            });
        }
    }

    impl MenuTarget {
        #[unsafe(method(performMenuAction:))]
        fn perform_menu_action(&self, sender: &NSMenuItem) {
            let action_identifier = sender.tag();
            super::platform::with_active_platform(|platform| {
                platform.perform_menu_action(action_identifier as usize);
            });
        }
    }
);

impl MenuTarget {
    fn new(main_thread_marker: MainThreadMarker) -> Retained<Self> {
        let this = MenuTarget::alloc(main_thread_marker).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) fn install_menus(menus: Vec<OwnedMenu>, keymap: &Keymap) {
    let Some(main_thread_marker) = MainThreadMarker::new() else {
        return;
    };

    let application = NSApplication::sharedApplication(main_thread_marker);
    let target = MenuTarget::new(main_thread_marker);
    let main_menu = menu(ns_string!("Main"), main_thread_marker);
    let mut actions = Vec::new();

    build_application_menu(&application, &main_menu, main_thread_marker);

    for owned_menu in &menus {
        let title = NSString::from_str(owned_menu.name.as_ref());
        let menu_item = empty_menu_item(title.as_ref(), main_thread_marker);
        let submenu = build_owned_menu(
            owned_menu,
            &target,
            keymap,
            &mut actions,
            main_thread_marker,
        );
        menu_item.setSubmenu(Some(&submenu));
        main_menu.addItem(&menu_item);

        if owned_menu.name.as_ref() == "Window" {
            application.setWindowsMenu(Some(&submenu));
        }
    }

    application.setMainMenu(Some(&main_menu));

    MENU_RUNTIME.with(|runtime| {
        *runtime.borrow_mut() = Some(MenuRuntime {
            _main_menu: main_menu,
            _target: target,
            actions,
        });
    });
}

pub(crate) fn with_action<R>(
    action_identifier: usize,
    f: impl FnOnce(&dyn Action) -> R,
) -> Option<R> {
    MENU_RUNTIME.with(|runtime| {
        let runtime = runtime.borrow();
        let runtime = runtime.as_ref()?;
        let action = runtime.actions.get(action_identifier)?;
        Some(f(action.as_ref()))
    })
}

fn build_application_menu(
    application: &NSApplication,
    main_menu: &NSMenu,
    main_thread_marker: MainThreadMarker,
) {
    let app_menu_item = empty_menu_item(ns_string!("Application"), main_thread_marker);
    let app_menu = menu(ns_string!("Application"), main_thread_marker);
    let process_name = NSProcessInfo::processInfo().processName();

    let about_title = ns_string!("About ").stringByAppendingString(&process_name);
    app_menu.addItem(&menu_item(
        &about_title,
        Some(sel!(orderFrontStandardAboutPanel:)),
        None,
        None,
        false,
        main_thread_marker,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(main_thread_marker));

    let services_menu = menu(ns_string!("Services"), main_thread_marker);
    let services_item = menu_item(
        ns_string!("Services"),
        None,
        None,
        None,
        false,
        main_thread_marker,
    );
    services_item.setSubmenu(Some(&services_menu));
    app_menu.addItem(&services_item);
    application.setServicesMenu(Some(&services_menu));

    let hide_title = ns_string!("Hide ").stringByAppendingString(&process_name);
    app_menu.addItem(&menu_item(
        &hide_title,
        Some(sel!(hide:)),
        Some("h"),
        Some(Modifiers::command()),
        false,
        main_thread_marker,
    ));
    app_menu.addItem(&menu_item(
        ns_string!("Hide Others"),
        Some(sel!(hideOtherApplications:)),
        Some("h"),
        Some(Modifiers {
            alt: true,
            platform: true,
            ..Default::default()
        }),
        false,
        main_thread_marker,
    ));
    app_menu.addItem(&menu_item(
        ns_string!("Show All"),
        Some(sel!(unhideAllApplications:)),
        None,
        None,
        false,
        main_thread_marker,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(main_thread_marker));

    let quit_title = ns_string!("Quit ").stringByAppendingString(&process_name);
    app_menu.addItem(&menu_item(
        &quit_title,
        Some(sel!(terminate:)),
        Some("q"),
        Some(Modifiers::command()),
        false,
        main_thread_marker,
    ));

    app_menu_item.setSubmenu(Some(&app_menu));
    main_menu.addItem(&app_menu_item);
}

fn build_owned_menu(
    owned_menu: &OwnedMenu,
    target: &MenuTarget,
    keymap: &Keymap,
    actions: &mut Vec<Box<dyn Action>>,
    main_thread_marker: MainThreadMarker,
) -> Retained<NSMenu> {
    let title = NSString::from_str(owned_menu.name.as_ref());
    let menu = menu(title.as_ref(), main_thread_marker);
    menu.setDelegate(Some(ProtocolObject::from_ref(target)));
    for owned_menu_item in &owned_menu.items {
        let item = build_menu_item(owned_menu_item, target, keymap, actions, main_thread_marker);
        menu.addItem(&item);
    }
    menu
}

fn build_menu_item(
    owned_menu_item: &OwnedMenuItem,
    target: &MenuTarget,
    keymap: &Keymap,
    actions: &mut Vec<Box<dyn Action>>,
    main_thread_marker: MainThreadMarker,
) -> Retained<NSMenuItem> {
    match owned_menu_item {
        OwnedMenuItem::Separator => NSMenuItem::separatorItem(main_thread_marker),
        OwnedMenuItem::SystemMenu(system_menu) => match system_menu.menu_type {
            SystemMenuType::Services => {
                let title = NSString::from_str(system_menu.name.as_ref());
                let item = empty_menu_item(title.as_ref(), main_thread_marker);
                let submenu = menu(title.as_ref(), main_thread_marker);
                item.setSubmenu(Some(&submenu));
                item
            }
        },
        OwnedMenuItem::Submenu(submenu) => {
            let title = NSString::from_str(submenu.name.as_ref());
            let item = empty_menu_item(title.as_ref(), main_thread_marker);
            let submenu = build_owned_menu(submenu, target, keymap, actions, main_thread_marker);
            item.setSubmenu(Some(&submenu));
            item
        }
        OwnedMenuItem::Action {
            name,
            action,
            checked,
            ..
        } => {
            let (key_equivalent, modifiers) = key_equivalent_for_action(action.as_ref(), keymap);
            let title = NSString::from_str(name);
            let item = menu_item(
                title.as_ref(),
                Some(sel!(performMenuAction:)),
                key_equivalent.as_deref(),
                modifiers,
                *checked,
                main_thread_marker,
            );
            let action_identifier = actions.len();
            actions.push(action.boxed_clone());
            unsafe {
                item.setTarget(Some(target));
                item.setTag(action_identifier as NSInteger);
            }
            item
        }
    }
}

fn key_equivalent_for_action(
    action: &dyn Action,
    keymap: &Keymap,
) -> (Option<String>, Option<Modifiers>) {
    let Some(binding) = keymap.bindings_for_action(action).next_back() else {
        return (None, None);
    };
    if binding.keystrokes().len() != 1 {
        return (None, None);
    }

    let keystroke = &binding.keystrokes()[0];
    let key = match keystroke.key() {
        "space" => " ".to_string(),
        key if key.len() == 1 => key.to_ascii_lowercase(),
        _ => return (None, None),
    };
    (Some(key), Some(*keystroke.modifiers()))
}

fn menu(title: &NSString, main_thread_marker: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::new(main_thread_marker);
    menu.setTitle(title);
    menu
}

fn empty_menu_item(title: &NSString, main_thread_marker: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(main_thread_marker);
    unsafe {
        item.setTitle(title);
        item.setAction(None);
        item.setKeyEquivalent(ns_string!(""));
    }
    item
}

fn menu_item(
    title: &NSString,
    selector: Option<Sel>,
    key_equivalent: Option<&str>,
    modifiers: Option<Modifiers>,
    checked: bool,
    main_thread_marker: MainThreadMarker,
) -> Retained<NSMenuItem> {
    let key_equivalent = key_equivalent
        .map(NSString::from_str)
        .unwrap_or_else(|| NSString::from_str(""));
    let item = NSMenuItem::new(main_thread_marker);
    unsafe {
        item.setTitle(title);
        item.setAction(selector);
        item.setKeyEquivalent(key_equivalent.as_ref());
    }
    item.setKeyEquivalentModifierMask(event_modifiers(modifiers.unwrap_or_default()));
    item.setState(if checked {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
    item
}

fn event_modifiers(modifiers: Modifiers) -> NSEventModifierFlags {
    let mut flags = NSEventModifierFlags::empty();
    if modifiers.control {
        flags |= NSEventModifierFlags::Control;
    }
    if modifiers.alt {
        flags |= NSEventModifierFlags::Option;
    }
    if modifiers.shift {
        flags |= NSEventModifierFlags::Shift;
    }
    if modifiers.platform {
        flags |= NSEventModifierFlags::Command;
    }
    if modifiers.function {
        flags |= NSEventModifierFlags::Function;
    }
    flags
}
