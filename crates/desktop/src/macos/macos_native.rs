use gpui::Window;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSAlert, NSApplication, NSImage, NSView, NSWindow};
use objc2_foundation::{NSError, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::Cell, path::PathBuf, rc::Rc};
use wry::{WebView, WebViewExtMacOS};

thread_local! {
    static TERMINATION: Termination = const { Termination {
        requested: Cell::new(false),
        ready: Cell::new(false),
        deferred: Cell::new(0),
    }};
}
struct Termination {
    requested: Cell<bool>,
    ready: Cell<bool>,
    deferred: Cell<u32>,
}

/// GPUI 0.2.2 clears its windows before waiting at most 100 ms for quit observers.
/// Install the earlier AppKit gate before opening any resource. We cancel the first
/// termination, keep the normal run loop running, and retry only after cleanup.
pub fn install_termination_gate() -> Result<(), &'static str> {
    use objc2::{
        runtime::{AnyClass, AnyObject, Imp, Sel},
        sel,
    };
    let mtm = MainThreadMarker::new().ok_or("The application must start on the main thread.")?;
    let delegate = NSApplication::sharedApplication(mtm)
        .delegate()
        .ok_or("The application termination gate could not be installed.")?;
    let delegate_object: &AnyObject = (*delegate).as_ref();
    let class = delegate_object.class();
    let selector = sel!(applicationShouldTerminate:);
    if class.name() != c"GPUIApplicationDelegate" || class.instance_method(selector).is_some() {
        return Err("The application delegate is incompatible with safe session shutdown.");
    }
    extern "C-unwind" fn should_terminate(_: &AnyObject, _: Sel, _: &NSApplication) -> usize {
        TERMINATION.with(|gate| {
            if gate.ready.get() {
                1 // NSTerminateNow
            } else {
                gate.requested.set(true);
                gate.deferred.set(gate.deferred.get().saturating_add(1));
                0 // NSTerminateCancel; a later ordinary terminate: retries after the fence.
            }
        })
    }
    // SAFETY: only our main-thread startup mutates this pinned GPUI delegate class.
    // The selector is absent, so no GPUI implementation is replaced. The callback
    // has the LP64 AppKit ABI: NSUInteger return, object, selector, NSApplication*.
    // Its code and method encoding live for the process lifetime; it only sets cells
    // and never reenters GPUI. Fail closed before any resource opens if installation fails.
    let installed = unsafe {
        objc2::ffi::class_addMethod(
            (class as *const AnyClass).cast_mut(),
            selector,
            std::mem::transmute::<
                extern "C-unwind" fn(&AnyObject, Sel, &NSApplication) -> usize,
                Imp,
            >(should_terminate),
            c"Q@:@".as_ptr(),
        )
    };
    if !installed.as_bool() {
        return Err("The application termination gate could not be installed.");
    }
    Ok(())
}

pub fn termination_requested() -> bool {
    TERMINATION.with(|gate| gate.requested.get())
}
pub fn allow_termination() {
    TERMINATION.with(|gate| gate.ready.set(true));
}
pub fn termination_deferrals() -> u32 {
    TERMINATION.with(|gate| gate.deferred.get())
}

/// Verification only: exercise AppKit's actual terminate: path outside a GPUI borrow.
pub fn terminate_application() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}

fn host_view(window: &Window) -> Option<&NSView> {
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
    else {
        return None;
    };
    // SAFETY: GPUI owns this main-thread NSView for the lifetime of the borrowed Window.
    Some(unsafe { handle.ns_view.cast::<NSView>().as_ref() })
}

pub fn parent_window(window: &Window) -> Option<Retained<NSWindow>> {
    host_view(window)?.window()
}

pub fn is_child(web: &WebView, window: &Window) -> bool {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return false;
    };
    // SAFETY: the WKWebView is alive and all callers run on GPUI's main thread.
    unsafe { web.webview().superview() }
        .is_some_and(|parent| Retained::as_ptr(&parent).cast() == handle.ns_view.as_ptr())
}

pub fn focus_dashboard(window: &Window) -> bool {
    let Some(view) = host_view(window) else {
        return false;
    };
    view.window()
        .is_some_and(|parent| parent.makeFirstResponder(Some(view)))
}
pub fn dashboard_has_focus(window: &Window) -> bool {
    let Some(view) = host_view(window) else {
        return false;
    };
    view.window()
        .and_then(|parent| parent.firstResponder())
        .is_some_and(|responder| {
            std::ptr::eq(
                Retained::as_ptr(&responder).cast::<()>(),
                (view as *const NSView).cast::<()>(),
            )
        })
}

pub fn game_has_focus(web: &WebView) -> bool {
    let Some(responder) = web.ns_window().firstResponder() else {
        return false;
    };
    let view = web.webview();
    responder
        .downcast_ref::<NSView>()
        .is_some_and(|focused| focused.isDescendantOf(&view))
}

pub fn confirm_mode(window: &Window, clientless: bool, result: Rc<Cell<Option<bool>>>) -> bool {
    let (Some(parent), Some(mtm)) = (parent_window(window), MainThreadMarker::new()) else {
        return false;
    };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(if clientless {
        "Switch to clientless mode?"
    } else {
        "Switch to client-assisted mode?"
    }));
    alert.setInformativeText(&NSString::from_str(
        "The current game or session will close. Sign in again in the selected mode.",
    ));
    alert
        .addButtonWithTitle(&NSString::from_str("Switch"))
        .setKeyEquivalent(&NSString::from_str("\r"));
    alert
        .addButtonWithTitle(&NSString::from_str("Cancel"))
        .setKeyEquivalent(&NSString::from_str("\u{1b}"));
    let callback = block2::RcBlock::new(move |response: isize| result.set(Some(response == 1000)));
    alert.beginSheetModalForWindow_completionHandler(&parent, Some(&callback));
    true
}

pub fn badge(show: bool) {
    if let Some(mtm) = MainThreadMarker::new() {
        let badge = NSString::from_str("!");
        NSApplication::sharedApplication(mtm)
            .dockTile()
            .setBadgeLabel(if show { Some(&badge) } else { None });
    }
}

/// Verification only. The path is supplied by the account-free verification runner.
pub fn snapshot(web: &WebView, path: PathBuf, completed: Rc<Cell<Option<bool>>>) {
    let callback = block2::RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
        // SAFETY: WebKit supplies a borrowed nullable NSImage for the duration of this callback.
        let image = if error.is_null() {
            unsafe { image.as_ref() }
        } else {
            None
        };
        let written = image
            .and_then(NSImage::TIFFRepresentation)
            .is_some_and(|data| std::fs::write(&path, data.to_vec()).is_ok());
        completed.set(Some(written));
    });
    // SAFETY: WKWebView is retained by the host, and WebKit copies the escaping completion block.
    unsafe {
        web.webview()
            .takeSnapshotWithConfiguration_completionHandler(None, &callback);
    }
}

pub fn menu_key(parent: &NSWindow, text: &str, key_code: u16, control: bool) -> bool {
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
    use objc2_foundation::NSPoint;
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let characters = NSString::from_str(text);
    let Some(event) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown, NSPoint::new(0.,0.), if control {NSEventModifierFlags::Control}else{NSEventModifierFlags::Command}, 0., parent.windowNumber(), None, &characters, &characters, false, key_code,
    ) else { return false; };
    NSApplication::sharedApplication(mtm)
        .mainMenu()
        .is_some_and(|menu| menu.performKeyEquivalent(&event))
}

pub fn page_key(parent: &NSWindow) -> bool {
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
    use objc2_foundation::NSPoint;
    let characters = NSString::from_str("x");
    let Some(event) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown, NSPoint::new(0.,0.), NSEventModifierFlags::empty(), 0., parent.windowNumber(), None, &characters, &characters, false, 7,
    ) else { return false; };
    parent.sendEvent(&event);
    true
}

pub fn has_sheet(window: &Window) -> bool {
    parent_window(window).is_some_and(|parent| parent.attachedSheet().is_some())
}

pub fn reduce_motion() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

pub fn test_appearance(window: &Window, dark: Option<bool>) -> bool {
    use objc2_app_kit::{
        NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    };
    let Some(parent) = parent_window(window) else {
        return false;
    };
    let appearance = dark.and_then(|dark| {
        // SAFETY: AppKit's exported appearance names are immutable process-lifetime constants.
        NSAppearance::appearanceNamed(unsafe {
            if dark {
                NSAppearanceNameDarkAqua
            } else {
                NSAppearanceNameAqua
            }
        })
    });
    if dark.is_some() && appearance.is_none() {
        return false;
    }
    parent.setAppearance(appearance.as_deref());
    true
}

pub fn has_badge() -> bool {
    MainThreadMarker::new().is_some_and(|mtm| {
        NSApplication::sharedApplication(mtm)
            .dockTile()
            .badgeLabel()
            .is_some_and(|label| label.to_string() == "!")
    })
}

pub fn appearance(window: &Window) -> Option<gpui::WindowAppearance> {
    use objc2_app_kit::{
        NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    };
    use objc2_foundation::NSArray;
    let parent = parent_window(window)?;
    // SAFETY: these immutable AppKit appearance names outlive the borrowed array elements.
    let (aqua, dark) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };
    let matched = parent
        .effectiveAppearance()
        .bestMatchFromAppearancesWithNames(&NSArray::from_slice(&[aqua, dark]))?;
    Some(if &*matched == dark {
        gpui::WindowAppearance::Dark
    } else {
        gpui::WindowAppearance::Light
    })
}

pub fn sheet_key(parent: &NSWindow, accept: bool) -> bool {
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
    use objc2_foundation::NSPoint;
    let Some(sheet) = parent.attachedSheet() else {
        return false;
    };
    let characters = NSString::from_str(if accept { "\r" } else { "\u{1b}" });
    let Some(event)=NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,NSPoint::new(0.,0.),NSEventModifierFlags::empty(),0.,sheet.windowNumber(),None,&characters,&characters,false,if accept{36}else{53},
    )else{return false;};
    sheet.performKeyEquivalent(&event)
}
