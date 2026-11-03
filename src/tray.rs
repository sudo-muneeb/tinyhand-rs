//! Modern StatusNotifier/DBusMenu integration. No XEmbed window is created.
use crate::desktop::Event;
use glib::variant::{ObjectPath, ToVariant};
use gtk::{gio, glib};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::Sender,
        Arc,
    },
};

const ITEM: &str = r#"<node><interface name="org.kde.StatusNotifierItem">
<method name="Activate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
<method name="SecondaryActivate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
<method name="ContextMenu"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
<property name="Category" type="s" access="read"/><property name="Id" type="s" access="read"/>
<property name="Title" type="s" access="read"/><property name="Status" type="s" access="read"/>
<property name="IconName" type="s" access="read"/><property name="ItemIsMenu" type="b" access="read"/>
<property name="WindowId" type="u" access="read"/><property name="IconThemePath" type="s" access="read"/><property name="IconPixmap" type="a(iiay)" access="read"/><property name="ToolTip" type="(sa(iiay)ss)" access="read"/><property name="Menu" type="o" access="read"/><signal name="NewStatus"><arg type="s"/></signal>
<signal name="NewIcon"/><signal name="NewToolTip"/>
</interface></node>"#;
const MENU: &str = r#"<node><interface name="com.canonical.dbusmenu">
<property name="Version" type="u" access="read"/><property name="TextDirection" type="s" access="read"/>
<property name="Status" type="s" access="read"/><property name="IconThemePath" type="as" access="read"/>
<method name="GetLayout"><arg type="i" direction="in"/><arg type="i" direction="in"/><arg type="as" direction="in"/>
<arg type="u" direction="out"/><arg type="(ia{sv}av)" direction="out"/></method>
<method name="GetGroupProperties"><arg type="ai" direction="in"/><arg type="as" direction="in"/><arg type="a(ia{sv})" direction="out"/></method>
<method name="GetProperty"><arg type="i" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method>
<method name="Event"><arg type="i" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="in"/><arg type="u" direction="in"/></method>
<method name="EventGroup"><arg type="a(isvu)" direction="in"/><arg type="ai" direction="out"/></method>
<method name="AboutToShow"><arg type="i" direction="in"/><arg type="b" direction="out"/></method>
<method name="AboutToShowGroup"><arg type="ai" direction="in"/><arg type="ai" direction="out"/><arg type="ai" direction="out"/></method>
<signal name="LayoutUpdated"><arg type="u"/><arg type="i"/></signal>
</interface></node>"#;

pub struct Tray {
    connection: gio::DBusConnection,
    owner: Option<gio::OwnerId>,
    watch: Option<Box<dyn FnOnce()>>,
    objects: Vec<gio::RegistrationId>,
    active: Arc<AtomicBool>,
    revision: Arc<AtomicU32>,
}
impl Tray {
    pub fn new(tx: Sender<Event>) -> Result<Self, glib::Error> {
        let connection = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)?;
        let active = Arc::new(AtomicBool::new(false));
        let revision = Arc::new(AtomicU32::new(1));
        let icons = icon_pixmaps();
        let theme = crate::render::asset_dir()
            .map(|p| p.join("icons").to_string_lossy().into_owned())
            .unwrap_or_default();
        let item = gio::DBusNodeInfo::for_xml(ITEM)?
            .lookup_interface("org.kde.StatusNotifierItem")
            .unwrap();
        let send = tx.clone();
        let item_id = connection.register_object(
            "/StatusNotifierItem",
            &item,
            move |_, _, _, _, method, _, reply| {
                let _ = send.send(if method == "Activate" {
                    Event::Toggle
                } else {
                    Event::Settings
                });
                reply.return_value(None);
            },
            move |_, _, _, _, name| match name {
                "Category" => "ApplicationStatus".to_variant(),
                "Id" => "tinyhand-rs".to_variant(),
                "Title" => "TinyHand".to_variant(),
                "Status" => "Active".to_variant(),
                "IconName" => "tinyhand-rs".to_variant(),
                "WindowId" => 0_u32.to_variant(),
                "IconThemePath" => theme.to_variant(),
                "IconPixmap" => icons.to_variant(),
                "ToolTip" => (
                    "tinyhand-rs",
                    icons.clone(),
                    "TinyHand",
                    "Click to start or stop presenting; open the menu for settings.",
                )
                    .to_variant(),
                "ItemIsMenu" => false.to_variant(),
                "Menu" => ObjectPath::try_from("/Menu").unwrap().to_variant(),
                _ => "".to_variant(),
            },
            |_, _, _, _, _, _| false,
        )?;
        let menu = gio::DBusNodeInfo::for_xml(MENU)?
            .lookup_interface("com.canonical.dbusmenu")
            .unwrap();
        let state = active.clone();
        let version = revision.clone();
        let send = tx.clone();
        let menu_id = connection.register_object(
            "/Menu",
            &menu,
            move |_, _, _, _, method, args, reply| {
                let on = state.load(Ordering::Relaxed);
                let value = match method {
                    "GetLayout" => {
                        let id = args.child_value(0).get::<i32>().unwrap_or(0);
                        glib::Variant::tuple_from_iter([
                            version.load(Ordering::Relaxed).to_variant(),
                            layout(id, on),
                        ])
                    }
                    "GetGroupProperties" => {
                        let ids = args.child_value(0).get::<Vec<i32>>().unwrap_or_default();
                        (ids.into_iter()
                            .map(|id| (id, properties(id, on)))
                            .collect::<Vec<_>>(),)
                            .to_variant()
                    }
                    "GetProperty" => {
                        let id = args.child_value(0).get::<i32>().unwrap_or(0);
                        let key = args.child_value(1).get::<String>().unwrap_or_default();
                        (properties(id, on)
                            .remove(&key)
                            .unwrap_or_else(|| "".to_variant()),)
                            .to_variant()
                    }
                    "Event" => {
                        if args.child_value(1).str() == Some("clicked") {
                            dispatch(&send, args.child_value(0).get::<i32>().unwrap_or(0));
                        }
                        ().to_variant()
                    }
                    "EventGroup" => {
                        let events = args.child_value(0);
                        for i in 0..events.n_children() {
                            let event = events.child_value(i);
                            if event.child_value(1).str() == Some("clicked") {
                                dispatch(&send, event.child_value(0).get::<i32>().unwrap_or(0));
                            }
                        }
                        (Vec::<i32>::new(),).to_variant()
                    }
                    "AboutToShow" => (false,).to_variant(),
                    "AboutToShowGroup" => (Vec::<i32>::new(), Vec::<i32>::new()).to_variant(),
                    _ => ().to_variant(),
                };
                reply.return_value(Some(&value));
            },
            |_, _, _, _, name| match name {
                "Version" => 3_u32.to_variant(),
                "TextDirection" => "ltr".to_variant(),
                "Status" => "normal".to_variant(),
                "IconThemePath" => Vec::<String>::new().to_variant(),
                _ => "".to_variant(),
            },
            |_, _, _, _, _, _| false,
        )?;
        let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
        let owner = gio::bus_own_name_on_connection(
            &connection,
            &name,
            gio::BusNameOwnerFlags::NONE,
            |_, _| {},
            |_, _| {},
        );
        let watch = gio::bus_watch_name_on_connection(
            &connection,
            "org.kde.StatusNotifierWatcher",
            gio::BusNameWatcherFlags::NONE,
            move |conn, _, _| {
                conn.call(
                    Some("org.kde.StatusNotifierWatcher"),
                    "/StatusNotifierWatcher",
                    "org.kde.StatusNotifierWatcher",
                    "RegisterStatusNotifierItem",
                    Some(&(name.clone(),).to_variant()),
                    None,
                    gio::DBusCallFlags::NONE,
                    3000,
                    None::<&gio::Cancellable>,
                    |r| {
                        if let Err(e) = r {
                            eprintln!("TinyHand tray registration: {e}");
                        }
                    },
                );
            },
            |_, _| {},
        );
        Ok(Self {
            connection,
            owner: Some(owner),
            watch: Some(Box::new(move || gio::bus_unwatch_name(watch))),
            objects: vec![item_id, menu_id],
            active,
            revision,
        })
    }
    pub fn set_active(&self, active: bool) {
        if self.active.swap(active, Ordering::Relaxed) != active {
            let _ = self.connection.emit_signal(
                None,
                "/Menu",
                "com.canonical.dbusmenu",
                "LayoutUpdated",
                Some(&(self.revision.fetch_add(1, Ordering::Relaxed) + 1, 0_i32).to_variant()),
            );
        }
    }
}
fn icon_pixmaps() -> Vec<(i32, i32, Vec<u8>)> {
    let Ok(path) = crate::render::asset_dir() else {
        return Vec::new();
    };
    let Ok(image) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(
        path.join("icons/tinyhand-rs.svg"),
        32,
        32,
        true,
    ) else {
        return Vec::new();
    };
    let data = image.read_pixel_bytes();
    let pixels = data.as_ref();
    let mut argb = Vec::with_capacity((image.width() * image.height() * 4) as usize);
    let channels = image.n_channels() as usize;
    for y in 0..image.height() as usize {
        for x in 0..image.width() as usize {
            let i = y * image.rowstride() as usize + x * channels;
            argb.extend_from_slice(&[
                if image.has_alpha() {
                    pixels[i + 3]
                } else {
                    255
                },
                pixels[i],
                pixels[i + 1],
                pixels[i + 2],
            ]);
        }
    }
    vec![(image.width(), image.height(), argb)]
}
fn dispatch(tx: &Sender<Event>, id: i32) {
    let event = match id {
        1 => Event::Toggle,
        2 => Event::Clear,
        3 => Event::Mute,
        4 => Event::Settings,
        5 => Event::Quit,
        _ => return,
    };
    let _ = tx.send(event);
}
fn properties(id: i32, active: bool) -> HashMap<String, glib::Variant> {
    let mut p = HashMap::new();
    p.insert("enabled".into(), true.to_variant());
    p.insert("visible".into(), true.to_variant());
    if id == 0 {
        p.insert("children-display".into(), "submenu".to_variant());
    } else {
        let title = match id {
            1 => {
                if active {
                    "Stop pointer"
                } else {
                    "Start pointer"
                }
            }
            2 => "Clear drawing",
            3 => "Toggle tap sound",
            4 => "Settings…",
            5 => "Quit TinyHand",
            _ => "",
        };
        p.insert("label".into(), title.to_variant());
    }
    p
}
fn layout(id: i32, active: bool) -> glib::Variant {
    let children = if id == 0 {
        (1..=5).map(|id| layout(id, active)).collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    (id, properties(id, active), children).to_variant()
}
impl Drop for Tray {
    fn drop(&mut self) {
        if let Some(w) = self.watch.take() {
            w();
        }
        if let Some(o) = self.owner.take() {
            gio::bus_unown_name(o);
        }
        for o in self.objects.drain(..) {
            let _ = self.connection.unregister_object(o);
        }
    }
}
