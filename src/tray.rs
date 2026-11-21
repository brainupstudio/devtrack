//! The DevTrack item in the macOS menu bar.
//!
//! - Left click shows the window (it may be closed: closing it keeps DevTrack running).
//! - Right click, or Control-click, opens the menu: new session, continue, stop, show, sync,
//!   Report Time, settings, quit.
//! - While a session runs, its time sits next to the mark ("0:52", hours and minutes).
//!
//! The mark is a template image: macOS draws it dark on a light menu bar and light on a dark
//! one. `assets/icons/tray-light.png` and `tray-dark.png` are the same drawing for systems
//! without template images (Windows, Linux), which have no item yet.
//!
//! AppKit calls back into one small Objective-C class declared here. Its handlers only store a
//! command and wake the interface, which carries the command out on its next frame, so the
//! data is only ever changed by the app itself. The time beside the mark is drawn by a timer of
//! its own, so it keeps counting while the window is closed.

use eframe::egui;
use objc2::encode::{Encode, Encoding};
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicPtr, AtomicU8, Ordering};
use std::sync::OnceLock;

#[repr(C)]
struct Size {
    w: f64,
    h: f64,
}

unsafe impl Encode for Size {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[Encoding::Double, Encoding::Double]);
}

#[link(name = "Foundation", kind = "framework")]
extern "C" {
    static NSRunLoopCommonModes: *mut AnyObject;
}

/// What the menu asked for, carried out by the app on its next frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Cmd {
    None = 0,
    Show,
    New,
    Continue,
    Stop,
    Sync,
    Report,
    Settings,
}

impl Cmd {
    fn from(v: u8) -> Cmd {
        match v {
            1 => Cmd::Show,
            2 => Cmd::New,
            3 => Cmd::Continue,
            4 => Cmd::Stop,
            5 => Cmd::Sync,
            6 => Cmd::Report,
            7 => Cmd::Settings,
            _ => Cmd::None,
        }
    }
    /// Commands that need the window in front.
    fn shows_window(self) -> bool {
        matches!(self, Cmd::Show | Cmd::New | Cmd::Report | Cmd::Settings)
    }
}

static ITEM: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());
static MENU: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());
/// The menu's lines, by command; 0 is the quit line.
static LINES: [AtomicPtr<AnyObject>; 8] = [const { AtomicPtr::new(std::ptr::null_mut()) }; 8];
static CMD: AtomicU8 = AtomicU8::new(0);
/// Start of the running session (Unix seconds), 0 when none runs.
static RUN_START: AtomicI64 = AtomicI64::new(0);
static CAN_CONTINUE: AtomicBool = AtomicBool::new(false);
static CAN_SYNC: AtomicBool = AtomicBool::new(false);
static CTX: OnceLock<egui::Context> = OnceLock::new();
static LAST_TITLE: AtomicI64 = AtomicI64::new(-1);
/// What the title shows: 0 the running session, 1 today's total, 2 nothing.
static TITLE_MODE: AtomicU8 = AtomicU8::new(0);
/// Finished time today, and when the running session's part of today began (0: none).
static TODAY_DONE: AtomicI64 = AtomicI64::new(0);
static TODAY_RUN_FROM: AtomicI64 = AtomicI64::new(0);

/// Side of the image in points: the menu bar is 22 high and the file is 44 px for 2x screens.
const SIDE: f64 = 22.0;
const NS_VARIABLE_LENGTH: f64 = -1.0;
const NS_IMAGE_LEFT: usize = 2;
const NS_RIGHT_MOUSE_DOWN: usize = 3;
const NS_RIGHT_MOUSE_UP: usize = 4;
const NS_CONTROL_KEY: usize = 1 << 18;
/// NSEventMaskLeftMouseUp | NSEventMaskRightMouseUp
const CLICK_MASK: usize = (1 << 2) | (1 << 4);

unsafe fn ns_string(s: &str) -> *mut AnyObject {
    let c = CString::new(s.replace('\0', "")).unwrap_or_default();
    msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()]
}

unsafe fn ns_app() -> *mut AnyObject {
    msg_send![class!(NSApplication), sharedApplication]
}

fn wake() {
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

/// Brings DevTrack and its window to the front, whether it was hidden, minimised or behind.
pub fn show_app() {
    unsafe {
        let app = ns_app();
        let _: () = msg_send![app, unhide: std::ptr::null_mut::<AnyObject>()];
        let _: () = msg_send![app, activateIgnoringOtherApps: true];
        let windows: *mut AnyObject = msg_send![app, windows];
        let n: usize = msg_send![windows, count];
        for i in 0..n {
            let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
            let mini: bool = msg_send![w, isMiniaturized];
            if mini {
                let _: () = msg_send![w, deminiaturize: std::ptr::null_mut::<AnyObject>()];
            }
            let _: () = msg_send![w, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
        }
    }
}

/// Hides DevTrack: the window goes, the item in the menu bar and the timers stay.
pub fn hide_app() {
    unsafe {
        let _: () = msg_send![ns_app(), hide: std::ptr::null_mut::<AnyObject>()];
    }
}

/// The command the menu asked for since the last frame, if any.
pub fn take_cmd() -> Cmd {
    Cmd::from(CMD.swap(0, Ordering::AcqRel))
}

/// What the item shows and offers: the running session's start, and whether Continue and Sync
/// have something to do. Called by the app on every frame.
pub fn set_state(running_start: Option<i64>, can_continue: bool, can_sync: bool) {
    RUN_START.store(running_start.unwrap_or(0), Ordering::Release);
    CAN_CONTINUE.store(can_continue, Ordering::Release);
    CAN_SYNC.store(can_sync, Ordering::Release);
}

/// What sits beside the mark, from the settings, and today's numbers it needs.
pub fn set_title_mode(mode: u8, done_today: i64, running_today_from: Option<i64>) {
    TITLE_MODE.store(mode.min(2), Ordering::Release);
    TODAY_DONE.store(done_today, Ordering::Release);
    TODAY_RUN_FROM.store(running_today_from.unwrap_or(0), Ordering::Release);
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// " 0:52" while a session runs (hours and minutes, like the timer's first digits), else empty.
fn title_now() -> (i64, String) {
    let mode = TITLE_MODE.load(Ordering::Acquire) as i64;
    let secs = match mode {
        // the running session
        0 => {
            let start = RUN_START.load(Ordering::Acquire);
            if start <= 0 {
                return (0, String::new());
            }
            now_secs() - start
        }
        // today's total, the running session included
        1 => {
            let from = TODAY_RUN_FROM.load(Ordering::Acquire);
            TODAY_DONE.load(Ordering::Acquire) + if from > 0 { now_secs() - from } else { 0 }
        }
        _ => return (-2, String::new()),
    };
    if secs <= 0 {
        return (-3 - mode, String::new());
    }
    let m = secs.max(0) / 60;
    // the key changes with the minute and with the mode, so the button is redrawn only then
    (1 + m + mode * 10_000_000, format!(" {}:{:02}", m / 60, m % 60))
}

unsafe fn refresh_title() {
    let item = ITEM.load(Ordering::Acquire);
    if item.is_null() {
        return;
    }
    let (key, text) = title_now();
    // the button is only touched when the minute changes
    if LAST_TITLE.swap(key, Ordering::AcqRel) == key {
        return;
    }
    let button: *mut AnyObject = msg_send![item, button];
    if !button.is_null() {
        let _: () = msg_send![button, setTitle: ns_string(&text)];
    }
}

/// The menu's words in the current language, and which lines can be used now.
unsafe fn refresh_menu() {
    use crate::ui::i18n::tr;
    let running = RUN_START.load(Ordering::Acquire) > 0;
    let lines: [(usize, &str, bool); 8] = [
        (0, "Quit DevTrack", true),
        (Cmd::Show as usize, "Show DevTrack", true),
        (Cmd::New as usize, "New session", !running),
        (Cmd::Continue as usize, "Continue the last session", !running && CAN_CONTINUE.load(Ordering::Acquire)),
        (Cmd::Stop as usize, "Stop", running),
        (Cmd::Sync as usize, "Sync now", CAN_SYNC.load(Ordering::Acquire)),
        (Cmd::Report as usize, "Report Time", true),
        (Cmd::Settings as usize, "Settings…", true),
    ];
    for (i, name, on) in lines {
        let line = LINES[i].load(Ordering::Acquire);
        if !line.is_null() {
            let _: () = msg_send![line, setTitle: ns_string(&tr(name))];
            let _: () = msg_send![line, setEnabled: on];
        }
    }
}

extern "C" fn on_click(_this: *mut AnyObject, _sel: Sel, _sender: *mut AnyObject) {
    unsafe {
        let ev: *mut AnyObject = msg_send![ns_app(), currentEvent];
        let (kind, flags): (usize, usize) = if ev.is_null() { (0, 0) } else { (msg_send![ev, type], msg_send![ev, modifierFlags]) };
        let menu_click = kind == NS_RIGHT_MOUSE_UP || kind == NS_RIGHT_MOUSE_DOWN || flags & NS_CONTROL_KEY != 0;
        if menu_click {
            let item = ITEM.load(Ordering::Acquire);
            let menu = MENU.load(Ordering::Acquire);
            if item.is_null() || menu.is_null() {
                return;
            }
            refresh_menu();
            // the menu opens under the item and the call returns once it closes
            let _: () = msg_send![item, setMenu: menu];
            let button: *mut AnyObject = msg_send![item, button];
            let _: () = msg_send![button, performClick: std::ptr::null_mut::<AnyObject>()];
            let _: () = msg_send![item, setMenu: std::ptr::null_mut::<AnyObject>()];
        } else {
            show_app();
            CMD.store(Cmd::Show as u8, Ordering::Release);
            wake();
        }
    }
}

extern "C" fn on_menu(_this: *mut AnyObject, _sel: Sel, sender: *mut AnyObject) {
    unsafe {
        let tag: isize = if sender.is_null() { 0 } else { msg_send![sender, tag] };
        let cmd = Cmd::from(tag.clamp(0, 255) as u8);
        if cmd == Cmd::None {
            return;
        }
        if cmd.shows_window() {
            show_app();
        }
        CMD.store(cmd as u8, Ordering::Release);
        wake();
    }
}

extern "C" fn on_tick(_this: *mut AnyObject, _sel: Sel, _timer: *mut AnyObject) {
    unsafe { refresh_title() }
}

fn target_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let mut b = ClassBuilder::new("DevTrackStatusTarget", class!(NSObject)).expect("class name is free");
        unsafe {
            b.add_method(sel!(devtrackClick:), on_click as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject));
            b.add_method(sel!(devtrackMenu:), on_menu as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject));
            b.add_method(sel!(devtrackTick:), on_tick as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject));
        }
        b.register()
    })
}

unsafe fn menu_line(menu: *mut AnyObject, target: *mut AnyObject, tag: usize, quit: bool) {
    let action = if quit { sel!(terminate:) } else { sel!(devtrackMenu:) };
    let item: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
    let item: *mut AnyObject = msg_send![item, initWithTitle: ns_string(""), action: action, keyEquivalent: ns_string("")];
    if item.is_null() {
        return;
    }
    if !quit {
        let _: () = msg_send![item, setTarget: target];
        let _: () = msg_send![item, setTag: tag as isize];
    }
    let _: () = msg_send![menu, addItem: item];
    LINES[if quit { 0 } else { tag }].store(item, Ordering::Release);
}

unsafe fn separator(menu: *mut AnyObject) {
    let s: *mut AnyObject = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![menu, addItem: s];
}

/// Adds the item to the menu bar. Call once, on the main thread, after the application exists.
/// The item, its menu and its timer live as long as the app does.
pub fn install(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
    static PNG: &[u8] = include_bytes!("../assets/icons/tray-template.png");
    unsafe {
        let target: *mut AnyObject = msg_send![target_class(), new];
        let data: *mut AnyObject = msg_send![class!(NSData), dataWithBytes: PNG.as_ptr().cast::<std::ffi::c_void>(), length: PNG.len()];
        let image: *mut AnyObject = msg_send![class!(NSImage), alloc];
        let image: *mut AnyObject = msg_send![image, initWithData: data];
        if image.is_null() || target.is_null() {
            return;
        }
        let _: () = msg_send![image, setSize: Size { w: SIDE, h: SIDE }];
        let _: () = msg_send![image, setTemplate: true];

        let bar: *mut AnyObject = msg_send![class!(NSStatusBar), systemStatusBar];
        let item: *mut AnyObject = msg_send![bar, statusItemWithLength: NS_VARIABLE_LENGTH];
        if item.is_null() {
            return;
        }
        let _: *mut AnyObject = msg_send![item, retain];
        let button: *mut AnyObject = msg_send![item, button];
        if !button.is_null() {
            let _: () = msg_send![button, setImage: image];
            let _: () = msg_send![button, setImagePosition: NS_IMAGE_LEFT];
            // digits of equal width, so the time does not shift as it counts
            let font: *mut AnyObject = msg_send![class!(NSFont), monospacedDigitSystemFontOfSize: 13.0f64, weight: 0.0f64];
            if !font.is_null() {
                let _: () = msg_send![button, setFont: font];
            }
            let _: () = msg_send![button, setToolTip: ns_string("DevTrack")];
            let _: () = msg_send![button, setTarget: target];
            let _: () = msg_send![button, setAction: sel!(devtrackClick:)];
            let _: isize = msg_send![button, sendActionOn: CLICK_MASK];
        }
        ITEM.store(item, Ordering::Release);

        let menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
        let menu: *mut AnyObject = msg_send![menu, init];
        let _: () = msg_send![menu, setAutoenablesItems: false];
        menu_line(menu, target, Cmd::New as usize, false);
        menu_line(menu, target, Cmd::Continue as usize, false);
        menu_line(menu, target, Cmd::Stop as usize, false);
        separator(menu);
        menu_line(menu, target, Cmd::Show as usize, false);
        separator(menu);
        menu_line(menu, target, Cmd::Sync as usize, false);
        menu_line(menu, target, Cmd::Report as usize, false);
        menu_line(menu, target, Cmd::Settings as usize, false);
        separator(menu);
        menu_line(menu, target, 0, true);
        MENU.store(menu, Ordering::Release);

        // the time beside the mark: checked every second, in every run-loop mode, so it keeps
        // counting while a menu is open or the window is closed
        let timer: *mut AnyObject = msg_send![class!(NSTimer), timerWithTimeInterval: 1.0f64, target: target, selector: sel!(devtrackTick:), userInfo: std::ptr::null_mut::<AnyObject>(), repeats: true];
        let run_loop: *mut AnyObject = msg_send![class!(NSRunLoop), mainRunLoop];
        let _: () = msg_send![run_loop, addTimer: timer, forMode: NSRunLoopCommonModes];
    }
}

// ---------- open at login ----------

const LOGIN_LABEL: &str = "studio.brainup.devtrack.login";

fn login_plist() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join("Library/LaunchAgents").join(format!("{LOGIN_LABEL}.plist")))
}

/// The DevTrack.app this process runs from, when it runs from one.
pub fn bundle_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let app = exe.parent()?.parent()?.parent()?;
    (app.extension().and_then(|e| e.to_str()) == Some("app")).then(|| app.to_path_buf())
}

/// Whether DevTrack opens at login; None when this copy is not an installed app.
pub fn opens_at_login() -> Option<bool> {
    bundle_path()?;
    Some(login_plist().is_some_and(|p| p.exists()))
}

/// Turns opening at login on or off. It opens hidden (`--background`): only the menu bar item
/// shows until the person clicks it.
pub fn set_open_at_login(on: bool) -> Result<(), String> {
    let plist = login_plist().ok_or("No home folder")?;
    if !on {
        return match std::fs::remove_file(&plist) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        };
    }
    let app = bundle_path().ok_or("Only the app installed in Applications can open at login")?;
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{LOGIN_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/bin/open</string>
        <string>-g</string>
        <string>-a</string>
        <string>{}</string>
        <string>--args</string>
        <string>--background</string>
    </array>
    <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
        esc(&app.to_string_lossy())
    );
    if let Some(dir) = plist.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&plist, body).map_err(|e| e.to_string())
}
