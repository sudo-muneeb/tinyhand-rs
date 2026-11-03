//! Native, keyboard-accessible GTK settings. No presentation grab is held here.
use crate::{
    config::{self, Config, STYLES},
    desktop::{self, Event},
    hands::{self, Library, PreparedHand},
};
use gtk::gdk::prelude::GdkContextExt;
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::mpsc::Sender,
};

pub struct SettingsWindow {
    pub window: gtk::Window,
    body: gtk::Box,
    tx: Sender<Event>,
    model: Rc<RefCell<Config>>,
    status: gtk::Label,
    stack: gtk::Stack,
    draft: Rc<RefCell<Option<PreparedHand>>>,
    preview: gtk::Image,
    add: gtk::Button,
    name: gtk::Entry,
    tab: String,
    chooser: Rc<RefCell<Option<gtk::FileChooserNative>>>,
    thumbnails: Thumbnails,
    pointer_buttons: Vec<(String, gtk::Button)>,
    appearance: Option<gtk::DrawingArea>,
    scrollers: HashMap<String, gtk::ScrolledWindow>,
}
type Model = Rc<RefCell<Config>>;
type Thumbnails = Rc<RefCell<HashMap<String, gtk::gdk_pixbuf::Pixbuf>>>;

impl SettingsWindow {
    pub fn new(tx: Sender<Event>) -> Self {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("TinyHand Settings");
        window.set_position(gtk::WindowPosition::Center);
        let work = gtk::gdk::Display::default()
            .and_then(|d| d.primary_monitor())
            .map(|m| m.workarea());
        let max_width = work.as_ref().map_or(900, |r| (r.width() - 32).max(360));
        let max_height = work.as_ref().map_or(720, |r| (r.height() - 32).max(360));
        window.set_default_size(900.min(max_width), 720.min(max_height));
        window.set_size_request(740.min(max_width), 540.min(max_height));
        let css = gtk::CssProvider::new();
        css.load_from_data(include_bytes!("../assets/settings.css"))
            .expect("valid settings stylesheet");
        let header = gtk::HeaderBar::new();
        header.set_title(Some("TinyHand Settings"));
        header.set_show_close_button(true);
        window.set_titlebar(Some(&header));
        header.show_all();
        if let Some(screen) = gtk::prelude::GtkWindowExt::screen(&window) {
            gtk::StyleContext::add_provider_for_screen(
                &screen,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        window.style_context().add_class("tinyhand");
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(true);
        }
        window.connect_delete_event(|w, _| {
            w.hide();
            glib::Propagation::Stop
        });
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.add(&body);
        Self {
            window,
            body,
            tx,
            model: Rc::new(RefCell::new(Config::default())),
            status: gtk::Label::new(None),
            stack: gtk::Stack::new(),
            draft: Rc::new(RefCell::new(None)),
            preview: gtk::Image::new(),
            add: gtk::Button::with_label("Add hand"),
            name: gtk::Entry::new(),
            tab: "General".into(),
            chooser: Rc::new(RefCell::new(None)),
            thumbnails: Rc::new(RefCell::new(HashMap::new())),
            pointer_buttons: Vec::new(),
            appearance: None,
            scrollers: HashMap::new(),
        }
    }
    pub fn visible(&self) -> bool {
        self.window.is_visible()
    }
    pub fn message(&self, text: &str) {
        self.status.set_text(text);
    }
    pub fn show(&mut self, config: &Config, library: &Library) {
        if let Some(tab) = self.stack.visible_child_name() {
            self.tab = tab.to_string();
        }
        let scroll_positions = self
            .scrollers
            .iter()
            .map(|(name, s)| (name.clone(), s.vadjustment().value()))
            .collect::<HashMap<_, _>>();
        self.scrollers.clear();
        self.pointer_buttons.clear();
        self.appearance = None;
        for c in self.body.children() {
            self.body.remove(&c);
        }
        *self.model.borrow_mut() = config.clone();
        let horizontal = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        self.body.pack_start(&horizontal, true, true, 0);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 4);
        sidebar.set_width_request(180);
        sidebar.style_context().add_class("sidebar");
        horizontal.pack_start(&sidebar, false, false, 0);
        let brand = gtk::Box::new(gtk::Orientation::Horizontal, 9);
        brand.style_context().add_class("brand");
        if let Ok(path) = crate::render::asset_dir() {
            let icon = gtk::Image::from_file(path.join("icons/tinyhand-rs.svg"));
            icon.set_pixel_size(30);
            if let Ok(pix) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(
                path.join("icons/tinyhand-rs.svg"),
                30,
                30,
                true,
            ) {
                icon.set_from_pixbuf(Some(&pix));
            }
            brand.pack_start(&icon, false, false, 0);
        }
        let brand_title = label("TinyHand", false);
        brand_title.style_context().add_class("brand-title");
        brand.pack_start(&brand_title, false, false, 0);
        sidebar.pack_start(&brand, false, false, 0);
        let nav_title = label("SETTINGS", true);
        nav_title.style_context().add_class("nav-section");
        sidebar.pack_start(&nav_title, false, false, 0);
        self.stack = gtk::Stack::new();
        self.stack
            .set_transition_type(gtk::StackTransitionType::None);
        self.stack.set_transition_duration(120);
        horizontal.pack_start(&self.stack, true, true, 0);
        let pages = [
            "General",
            "Pointers",
            "Appearance",
            "Drawing",
            "Sound",
            "About",
        ];
        let mut first: Option<gtk::RadioButton> = None;
        for (index, title) in pages.iter().enumerate() {
            let b = if let Some(first) = &first {
                gtk::RadioButton::with_label_from_widget(first, title)
            } else {
                gtk::RadioButton::with_label(title)
            };
            b.set_mode(false);
            b.set_widget_name(&format!("tab-{title}"));
            b.style_context().add_class("nav");
            for child in b.children() {
                b.remove(&child);
            }
            let nav = gtk::Box::new(gtk::Orientation::Horizontal, 11);
            let symbols = [
                "preferences-system-symbolic",
                "input-mouse-symbolic",
                "applications-graphics-symbolic",
                "document-edit-symbolic",
                "audio-volume-high-symbolic",
                "help-about-symbolic",
            ];
            let icon = gtk::Image::from_icon_name(Some(symbols[index]), gtk::IconSize::Menu);
            icon.set_pixel_size(17);
            nav.pack_start(&icon, false, false, 0);
            nav.pack_start(&label(title, false), false, false, 0);
            b.add(&nav);
            if let Some(a) = b.accessible() {
                a.set_name(title);
            }
            if first.is_none() {
                first = Some(b.clone());
            }
            let stack = self.stack.clone();
            let page = title.to_string();
            b.connect_toggled(move |b| {
                if b.is_active() {
                    stack.set_visible_child_name(&page);
                }
            });
            sidebar.pack_start(&b, false, false, 0);
            let content = self.page(index, config, library);
            let scrolled =
                gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
            scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            scrolled.add(&content);
            if let Some(value) = scroll_positions.get(*title).copied().filter(|v| *v > 0.0) {
                let restored = Cell::new(false);
                scrolled.vadjustment().connect_changed(move |a| {
                    if !restored.get() && a.upper() > a.page_size() {
                        restored.set(true);
                        a.set_value(value.min(a.upper() - a.page_size()).max(0.0));
                    }
                });
            }
            self.scrollers.insert(title.to_string(), scrolled.clone());
            self.stack.add_titled(&scrolled, title, title);
            if self.tab == *title {
                b.set_active(true);
            }
        }
        let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar.pack_start(&spacer, true, true, 0);
        let version = label(concat!("Version ", env!("CARGO_PKG_VERSION")), true);
        version.style_context().add_class("version");
        sidebar.pack_end(&version, false, false, 0);
        let quit = gtk::Button::with_label("Quit TinyHand");
        quit.style_context().add_class("quiet");
        let tx = self.tx.clone();
        quit.connect_clicked(move |_| {
            let _ = tx.send(Event::Quit);
        });
        sidebar.pack_end(&quit, false, false, 0);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.style_context().add_class("footer");
        self.status = gtk::Label::new(Some("Changes are saved automatically"));
        self.status.set_xalign(0.0);
        self.status.set_line_wrap(true);
        self.status.set_max_width_chars(60);
        self.status.style_context().add_class("muted");
        footer.pack_start(&self.status, true, true, 0);
        let start = gtk::Button::with_label("Start pointer");
        start.style_context().add_class("primary");
        let tx = self.tx.clone();
        start.connect_clicked(move |_| {
            let _ = tx.send(Event::Start);
        });
        footer.pack_end(&start, false, false, 0);
        self.body.pack_end(&footer, false, false, 0);
        self.body.show_all();
        self.stack.set_visible_child_name(&self.tab);
        self.window.present();
        gtk::prelude::GtkWindowExt::set_focus(&self.window, Some(&start));
    }
    fn page(&mut self, index: usize, cfg: &Config, library: &Library) -> gtk::Box {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 22);
        page.style_context().add_class("page");
        let title = gtk::Label::new(Some(
            [
                "General",
                "Pointers",
                "Appearance",
                "Drawing",
                "Sound",
                "About",
            ][index],
        ));
        title.set_xalign(0.0);
        title.style_context().add_class("heading");
        let heading = gtk::Box::new(gtk::Orientation::Vertical, 6);
        heading.pack_start(&title, false, false, 0);
        let descriptions = [
            "Make TinyHand work the way you present.",
            "Choose your favourites and make the toolbar your own.",
            "A little personality, just the way you like it.",
            "Make your ideas clear with colour and ink.",
            "Find the right sound for every tap.",
            "A tiny hand. A little more personality.",
        ];
        let description = label(descriptions[index], true);
        description.style_context().add_class("page-description");
        heading.pack_start(&description, false, false, 0);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        header.pack_start(&heading, true, true, 0);
        page.pack_start(&header, false, false, 0);
        match index {
            0 => {
                let shortcuts = card(&page, "Shortcuts");
                self.shortcut(&shortcuts, "Turn the pointer on or off", true, &cfg.toggle);
                self.shortcut(&shortcuts, "Clear the drawing", false, &cfg.clear);
                let interaction = card(&page, "Clicking for real");
                let keys = gtk::ComboBoxText::new();
                for s in ["Alt", "Control", "Super", "Off"] {
                    keys.append(Some(s), s);
                }
                keys.set_active_id(Some(&cfg.interact_key));
                row(
                    &interaction,
                    "Hold to use the app",
                    "Clicks, drags and scrolling reach the underlying app.",
                    &keys,
                );
                let m = self.model.clone();
                let tx = self.tx.clone();
                keys.connect_changed(move |w| {
                    if let Some(s) = w.active_id() {
                        change(&m, &tx, |c| c.interact_key = s.to_string());
                    }
                });
                switch(
                    &interaction,
                    "Keep the hand while interacting",
                    "Otherwise the normal cursor comes back.",
                    cfg.interact_keeps_hand,
                    &self.model,
                    &self.tx,
                    |c, v| c.interact_keeps_hand = v,
                );
                let startup = card(&page, "Startup");
                switch(
                    &startup,
                    "Open TinyHand when you log in",
                    "",
                    cfg.startup,
                    &self.model,
                    &self.tx,
                    |c, v| c.startup = v,
                );
            }
            1 => {
                let pins = card(&page, "In your toolbar");
                let slots = gtk::Box::new(gtk::Orientation::Horizontal, 5);
                pins.pack_start(&slots, false, false, 0);
                for (slot, id) in cfg.pinned.iter().enumerate() {
                    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 2);
                    let button = pointer_button(id, library, &self.thumbnails);
                    if *id == cfg.selected_id() {
                        button.style_context().add_class("selected");
                    }
                    self.pointer_buttons.push((id.clone(), button.clone()));
                    if let Some(a) = button.accessible() {
                        a.set_name(&format!("Use {id}"));
                    }
                    let tx = self.tx.clone();
                    let pick = id.clone();
                    button.connect_clicked(move |_| {
                        let _ = tx.send(Event::Pick(pick.clone()));
                    });
                    button.set_tooltip_text(Some("Drag to reorder; drag outside to remove"));
                    let targets = [gtk::TargetEntry::new(
                        "application/x-tinyhand-pointer",
                        gtk::TargetFlags::SAME_APP,
                        0,
                    )];
                    button.drag_source_set(
                        gdk::ModifierType::BUTTON1_MASK,
                        &targets,
                        gdk::DragAction::MOVE,
                    );
                    let item = id.clone();
                    button.connect_drag_data_get(move |_, _, data, _, _| {
                        data.set_text(&item);
                    });
                    button.drag_dest_set(gtk::DestDefaults::ALL, &targets, gdk::DragAction::MOVE);
                    let tx = self.tx.clone();
                    button.connect_drag_data_received(move |_, context, _, _, data, _, time| {
                        if let Some(id) = data.text() {
                            let _ = tx.send(Event::Reorder(id.to_string(), slot));
                            context.drag_finish(true, false, time);
                        }
                    });
                    let tx = self.tx.clone();
                    let item = id.clone();
                    button.connect_drag_failed(move |_, _, result| {
                        if result == gtk::DragResult::NoTarget {
                            let _ = tx.send(Event::Unpin(item.clone()));
                        }
                        glib::Propagation::Proceed
                    });
                    let remove = gtk::Button::from_icon_name(
                        Some("window-close-symbolic"),
                        gtk::IconSize::Menu,
                    );
                    remove.style_context().add_class("remove-pin");
                    if let Some(a) = remove.accessible() {
                        a.set_name(&format!("Remove {id}"));
                    }
                    remove.set_tooltip_text(Some("Remove from toolbar"));
                    let tx = self.tx.clone();
                    let item = id.clone();
                    remove.connect_clicked(move |_| {
                        let _ = tx.send(Event::Unpin(item.clone()));
                    });
                    box_.pack_start(&button, false, false, 0);
                    box_.pack_start(&remove, false, false, 0);
                    slots.pack_start(&box_, false, false, 0);
                }
                let hint = gtk::Label::new(Some(
                    "Drag to reorder, or out to remove. Keep at least one; up to eight.",
                ));
                hint.set_line_wrap(true);
                hint.style_context().add_class("muted");
                page.pack_start(&hint, false, false, 0);
                let flow = gtk::FlowBox::new();
                flow.set_selection_mode(gtk::SelectionMode::None);
                flow.set_max_children_per_line(7);
                flow.set_min_children_per_line(4);
                flow.set_column_spacing(7);
                flow.set_row_spacing(7);
                for id in STYLES
                    .iter()
                    .map(|s| s.to_string())
                    .chain(library.hands.iter().map(|h| h.id.clone()))
                {
                    let b = pointer_tile(
                        &id,
                        library,
                        &self.thumbnails,
                        true,
                        cfg.pinned.contains(&id),
                    );
                    if id == cfg.selected_id() {
                        b.style_context().add_class("selected");
                    }
                    self.pointer_buttons.push((id.clone(), b.clone()));
                    if let Some(a) = b.accessible() {
                        a.set_name(&format!("Pin {id}"));
                    }

                    let item = id.clone();
                    let pinned = cfg.pinned.contains(&id);
                    if !pinned && cfg.pinned.len() >= 8 {
                        let pop = gtk::Popover::new(Some(&b));
                        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
                        content.set_margin_top(12);
                        content.set_margin_bottom(12);
                        content.set_margin_start(12);
                        content.set_margin_end(12);
                        content.pack_start(
                            &label("The toolbar is full. Replace a pointer:", false),
                            false,
                            false,
                            0,
                        );
                        let choices = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                        content.pack_start(&choices, false, false, 0);
                        for old in &cfg.pinned {
                            let swap = pointer_button(old, library, &self.thumbnails);
                            if let Some(a) = swap.accessible() {
                                a.set_name(&format!("Replace {old}"));
                            }
                            let tx = self.tx.clone();
                            let old = old.clone();
                            let new = item.clone();
                            let pop = pop.clone();
                            swap.connect_clicked(move |_| {
                                pop.popdown();
                                let _ = tx.send(Event::Swap(old.clone(), new.clone()));
                            });
                            choices.pack_start(&swap, false, false, 0);
                        }
                        pop.add(&content);
                        b.connect_clicked(move |_| pop.show_all());
                    } else {
                        let tx = self.tx.clone();
                        b.connect_clicked(move |_| {
                            let _ = tx.send(if pinned {
                                Event::Unpin(item.clone())
                            } else {
                                Event::Pin(item.clone())
                            });
                        });
                    }
                    flow.insert(&b, -1);
                }
                let gallery = card(&page, "Pointer library");
                let help = label(
                    "Click to add or remove a pointer. A checkmark means it is in your toolbar.",
                    true,
                );
                help.set_margin_top(12);
                gallery.pack_start(&help, false, false, 0);
                gallery.pack_start(&flow, false, false, 12);
                if !library.hands.is_empty() {
                    let yours = card(&page, "Your hands");
                    for h in &library.hands {
                        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 5);
                        let use_ = gtk::Button::with_label("Use");
                        let tx = self.tx.clone();
                        let id = h.id.clone();
                        use_.connect_clicked(move |_| {
                            let _ = tx.send(Event::Pick(id.clone()));
                        });
                        controls.pack_start(&use_, false, false, 0);
                        let delete = gtk::Button::with_label("Delete");
                        let id = h.id.clone();
                        let tx = self.tx.clone();
                        delete.connect_clicked(move |_| {
                            let _ = tx.send(Event::RemoveHand(id.clone()));
                        });
                        controls.pack_start(&delete, false, false, 0);
                        row(&yours, &h.name, "", &controls);
                    }
                }
                let custom = gtk::Expander::new(Some("Make your own hand…"));
                custom.set_expanded(false);
                let create = gtk::Button::with_label("Create hand");
                create.set_valign(gtk::Align::Center);
                let expander = custom.clone();
                create.connect_clicked(move |_| {
                    expander.set_expanded(true);
                    expander.grab_focus();
                });
                header.pack_end(&create, false, false, 0);
                let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
                custom.add(&body);
                page.pack_start(&custom, false, false, 0);
                let idea = gtk::Entry::new();
                idea.set_placeholder_text(Some("e.g. knitted wool in mustard yellow"));
                body.pack_start(&label("1. Describe it", false), false, false, 0);
                body.pack_start(&idea, false, false, 0);
                body.pack_start(
                    &label(
                        "2. Make the image — transparent PNG, no shadow or stick",
                        true,
                    ),
                    false,
                    false,
                    0,
                );
                let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let browser = gtk::Button::with_label("Open in ChatGPT");
                let idea_ = idea.clone();
                let tx = self.tx.clone();
                browser.connect_clicked(move |_| {
                    let uri = format!(
                        "https://chatgpt.com/?q={}",
                        desktop::encoded(&hands::prompt(&idea_.text()))
                    );
                    if let Err(e) = desktop::launch(&uri) {
                        let _ = tx.send(Event::Message(e));
                    }
                });
                actions.pack_start(&browser, false, false, 0);
                let copy = gtk::Button::with_label("Copy prompt");
                let idea_ = idea.clone();
                copy.connect_clicked(move |b| {
                    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD)
                        .set_text(&hands::prompt(&idea_.text()));
                    b.set_label("Copied");
                    let b = b.clone();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(1600),
                        move || b.set_label("Copy prompt"),
                    );
                });
                actions.pack_start(&copy, false, false, 0);
                body.pack_start(&actions, false, false, 0);
                self.name = gtk::Entry::new();
                self.name.set_max_length(16);
                self.name.set_placeholder_text(Some("My hand"));
                row(&body, "Name", "", &self.name);
                let choose =
                    gtk::Button::with_label("3. Drop an image here, or click to choose it");
                choose.set_size_request(-1, 80);
                choose.style_context().add_class("drop-zone");
                let parent = self.window.clone();
                let tx = self.tx.clone();
                let name = self.name.clone();
                let chooser = self.chooser.clone();
                choose.connect_clicked(move |_| {
                    let dialog = gtk::FileChooserNative::new(
                        Some("Choose your TinyHand image"),
                        Some(&parent),
                        gtk::FileChooserAction::Open,
                        Some("Choose"),
                        Some("Cancel"),
                    );
                    let filter = gtk::FileFilter::new();
                    filter.set_name(Some("Images"));
                    filter.add_pixbuf_formats();
                    dialog.add_filter(filter);
                    let tx = tx.clone();
                    let name = name.clone();
                    *chooser.borrow_mut() = Some(dialog.clone());
                    let chooser = chooser.clone();
                    dialog.connect_response(move |d, r| {
                        if r == gtk::ResponseType::Accept {
                            if let Some(path) = d.filename() {
                                let _ = tx.send(Event::Prepare(path, name.text().to_string()));
                            }
                        }
                        d.destroy();
                        chooser.borrow_mut().take();
                    });
                    dialog.show();
                });
                choose.drag_dest_set(
                    gtk::DestDefaults::ALL,
                    &[gtk::TargetEntry::new(
                        "text/uri-list",
                        gtk::TargetFlags::OTHER_APP,
                        0,
                    )],
                    gdk::DragAction::COPY,
                );
                let tx = self.tx.clone();
                let name = self.name.clone();
                choose.connect_drag_data_received(move |_, ctx, _, _, data, _, time| {
                    if let Some(uri) = data.uris().first() {
                        if let Some(path) = gtk::gio::File::for_uri(uri).path() {
                            let _ = tx.send(Event::Prepare(path, name.text().to_string()));
                            ctx.drag_finish(true, false, time);
                            return;
                        }
                    }
                    ctx.drag_finish(false, false, time);
                });
                body.pack_start(&choose, false, false, 0);
                self.preview = gtk::Image::new();
                self.preview.style_context().add_class("preview");
                body.pack_start(&self.preview, false, false, 0);
                self.add = gtk::Button::with_label("Add hand");
                self.add.set_sensitive(false);
                self.add.style_context().add_class("primary");
                let draft = self.draft.clone();
                let tx = self.tx.clone();
                let name = self.name.clone();
                self.add.connect_clicked(move |_| {
                    if let Some(mut hand) = draft.borrow_mut().take() {
                        hand.name = name.text().to_string();
                        let _ = tx.send(Event::AddHand(hand));
                    }
                });
                body.pack_start(&self.add, false, false, 0);
                let reset = gtk::Button::with_label("Reset toolbar");
                let m = self.model.clone();
                let tx = self.tx.clone();
                reset.connect_clicked(move |_| {
                    change(&m, &tx, |c| c.pinned = config::default_pins())
                });
                reset.set_halign(gtk::Align::End);
                reset.style_context().add_class("quiet");
                page.pack_start(&reset, false, false, 0);
            }
            2 => {
                let area = appearance_preview(self.model.clone(), self.thumbnails.clone());
                area.style_context().add_class("preview-stage");
                self.appearance = Some(area.clone());
                page.pack_start(&area, false, false, 0);
                let group = card(&page, "Pointer");
                slider(
                    &group,
                    "Size",
                    cfg.size,
                    (0.08, 0.20, 0.01),
                    &self.model,
                    &self.tx,
                    |c, v| c.size = v,
                );
                switch(
                    &group,
                    "Shadow under the pointer",
                    "",
                    cfg.shadow,
                    &self.model,
                    &self.tx,
                    |c, v| c.shadow = v,
                );
                slider(
                    &group,
                    "Tilt",
                    cfg.angle,
                    (-1.2, 1.2, 0.04),
                    &self.model,
                    &self.tx,
                    |c, v| c.angle = v,
                );
                slider(
                    &group,
                    "Opacity",
                    cfg.opacity,
                    (0.2, 1.0, 0.05),
                    &self.model,
                    &self.tx,
                    |c, v| c.opacity = v,
                );
            }
            3 => {
                let group = card(&page, "Ink");
                let swatches = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                let first = gtk::RadioButton::with_label("Match");
                first.set_mode(false);
                first.set_valign(gtk::Align::Center);
                first.style_context().add_class("swatch");
                first.set_active(cfg.match_color);
                let m = self.model.clone();
                let tx = self.tx.clone();
                first.connect_toggled(move |w| {
                    if w.is_active() {
                        change(&m, &tx, |c| c.match_color = true);
                    }
                });
                swatches.pack_start(&first, false, false, 0);
                for (index, title) in [
                    (1, "Red"),
                    (2, "Orange"),
                    (5, "Yellow"),
                    (3, "Green"),
                    (4, "Blue"),
                    (0, "Purple"),
                    (6, "White"),
                    (7, "Black"),
                ] {
                    let b = gtk::RadioButton::with_label_from_widget(&first, "●");
                    b.set_mode(false);
                    b.set_valign(gtk::Align::Center);
                    b.style_context().add_class("swatch");
                    b.set_tooltip_text(Some(title));
                    if let Some(a) = b.accessible() {
                        a.set_name(title);
                    }
                    b.set_active(!cfg.match_color && index == cfg.color);
                    let (r, g, blue) = crate::colors::SWATCHES[index];
                    let css = gtk::CssProvider::new();
                    let _ = css.load_from_data(
                        format!(
                            "button{{color:rgb({},{},{});font-size:20px;}}",
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (blue * 255.0) as u8
                        )
                        .as_bytes(),
                    );
                    b.style_context()
                        .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
                    let m = self.model.clone();
                    let tx = self.tx.clone();
                    b.connect_toggled(move |w| {
                        if w.is_active() {
                            change(&m, &tx, |c| {
                                c.match_color = false;
                                c.color = index;
                            });
                        }
                    });
                    swatches.pack_start(&b, false, false, 0);
                }
                group.pack_start(&label("Ink colour", false), false, false, 4);
                group.pack_start(
                    &label("Matches each pointer, or choose a fixed colour.", true),
                    false,
                    false,
                    0,
                );
                group.pack_start(&swatches, false, false, 8);
                let fade = gtk::ComboBoxText::new();
                for (id, title) in [
                    ("3", "After 3 seconds"),
                    ("6", "After 6 seconds"),
                    ("10", "After 10 seconds"),
                    ("0", "Keep until cleared"),
                ] {
                    fade.append(Some(id), title);
                }
                let selected = if cfg.fade <= 0.0 {
                    "0".into()
                } else {
                    format!("{}", cfg.fade)
                };
                fade.set_active_id(Some(&selected));
                let m = self.model.clone();
                let tx = self.tx.clone();
                fade.connect_changed(move |w| {
                    if let Some(v) = w.active_id().and_then(|s| s.parse().ok()) {
                        change(&m, &tx, |c| c.fade = v);
                    }
                });
                row(
                    &group,
                    "Lines fade",
                    "Fade starts after the selected delay.",
                    &fade,
                );
                slider(
                    &group,
                    "Thickness",
                    cfg.thickness,
                    (1.0, 30.0, 1.0),
                    &self.model,
                    &self.tx,
                    |c, v| c.thickness = v,
                );
                switch(
                    &group,
                    "Ink button in the toolbar",
                    "Change colour while presenting.",
                    cfg.show_ink_button,
                    &self.model,
                    &self.tx,
                    |c, v| c.show_ink_button = v,
                );
                let clear = gtk::Button::with_label("Clear drawing");
                clear.set_halign(gtk::Align::End);
                clear.style_context().add_class("quiet");
                let tx = self.tx.clone();
                clear.connect_clicked(move |_| {
                    let _ = tx.send(Event::Clear);
                });
                page.pack_start(&clear, false, false, 0);
            }
            4 => {
                let group = card(&page, "Tap sound");
                let flow = gtk::FlowBox::new();
                flow.set_selection_mode(gtk::SelectionMode::None);
                flow.set_max_children_per_line(4);
                flow.set_min_children_per_line(3);
                flow.set_row_spacing(8);
                flow.set_column_spacing(8);
                let mut first: Option<gtk::RadioButton> = None;
                let symbols = [
                    "audio-volume-muted-symbolic",
                    "audio-volume-medium-symbolic",
                    "input-mouse-symbolic",
                    "media-record-symbolic",
                    "view-grid-symbolic",
                    "face-smile-symbolic",
                    "audio-volume-high-symbolic",
                ];
                for (kind, title) in crate::settings::SOUNDS.iter().enumerate() {
                    let b = if let Some(first) = &first {
                        gtk::RadioButton::from_widget(first)
                    } else {
                        gtk::RadioButton::new()
                    };
                    b.set_mode(false);
                    if first.is_none() {
                        first = Some(b.clone());
                    }
                    b.style_context().add_class("sound-tile");
                    if let Some(a) = b.accessible() {
                        a.set_name(title);
                    }
                    let content = gtk::Box::new(gtk::Orientation::Vertical, 9);
                    let icon = gtk::Image::from_icon_name(
                        Some(symbols[kind]),
                        gtk::IconSize::LargeToolbar,
                    );
                    icon.set_pixel_size(22);
                    content.pack_start(&icon, false, false, 0);
                    let title_ = label(title, false);
                    title_.set_xalign(0.5);
                    content.pack_start(&title_, false, false, 0);
                    b.add(&content);
                    b.set_active(
                        (kind == 0 && !cfg.sound) || (cfg.sound && kind == cfg.sound_kind),
                    );
                    let m = self.model.clone();
                    let tx = self.tx.clone();
                    b.connect_toggled(move |b| {
                        if b.is_active() {
                            change(&m, &tx, |c| {
                                c.sound = kind > 0;
                                if kind > 0 {
                                    c.sound_kind = kind;
                                }
                            });
                        }
                    });
                    let tx = self.tx.clone();
                    b.connect_clicked(move |_| {
                        if kind > 0 {
                            let _ = tx.send(Event::Message(format!("preview-sound:{kind}")));
                        }
                    });
                    flow.insert(&b, -1);
                }
                group.pack_start(&flow, false, false, 12);
                page.pack_start(
                    &label(
                        "Click a sound to hear it. It plays each time the hand taps.",
                        true,
                    ),
                    false,
                    false,
                    0,
                );
                let other = card(&page, "Playback");
                slider(
                    &other,
                    "Volume",
                    cfg.volume,
                    (0.0, 1.0, 0.05),
                    &self.model,
                    &self.tx,
                    |c, v| c.volume = v,
                );
                switch(
                    &other,
                    "Laser dot taps too",
                    "Off by default, for quiet pointing.",
                    cfg.laser_sound,
                    &self.model,
                    &self.tx,
                    |c, v| c.laser_sound = v,
                );
            }
            _ => {
                let about = card(&page, "");
                let hero = gtk::Box::new(gtk::Orientation::Horizontal, 18);
                hero.set_margin_top(18);
                hero.set_margin_bottom(18);
                if let Ok(path) = crate::render::asset_dir() {
                    if let Ok(pix) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(
                        path.join("icons/tinyhand-rs.svg"),
                        64,
                        64,
                        true,
                    ) {
                        hero.pack_start(&gtk::Image::from_pixbuf(Some(&pix)), false, false, 0);
                    }
                }
                let text = gtk::Box::new(gtk::Orientation::Vertical, 5);
                let name = label("TinyHand for Linux", false);
                name.style_context().add_class("brand-title");
                text.pack_start(&name, false, false, 0);
                text.pack_start(
                    &label(concat!("Version ", env!("CARGO_PKG_VERSION")), true),
                    false,
                    false,
                    0,
                );
                text.pack_start(
                    &label("Free, open source, and made for your presentations.", true),
                    false,
                    false,
                    0,
                );
                hero.pack_start(&text, true, true, 0);
                about.pack_start(&hero, false, false, 0);
                let links = card(&page, "Around TinyHand");
                for (title, detail, url) in [
                    ("Website", "tinyhand.app", "https://tinyhand.app"),
                    (
                        "Original source",
                        "nilbuild / tinyhand",
                        "https://github.com/nilbuild/tinyhand",
                    ),
                    ("Made by @nilbuild", "Follow on X", "https://x.com/nilbuild"),
                    (
                        "MIT license",
                        "Free and open source",
                        "https://github.com/nilbuild/tinyhand/blob/main/LICENSE",
                    ),
                ] {
                    let b = gtk::LinkButton::with_label(url, "Open ↗");
                    b.style_context().add_class("quiet");
                    row(&links, title, detail, &b);
                }
                let reset = gtk::Button::with_label("Reset all settings…");
                let parent = self.window.clone();
                let tx = self.tx.clone();
                reset.connect_clicked(move |_|{let d=gtk::MessageDialog::new(Some(&parent),gtk::DialogFlags::MODAL,gtk::MessageType::Question,gtk::ButtonsType::OkCancel,"Reset shortcuts, pointers, appearance, drawing and sound? Custom hands and startup preferences are kept.");let tx=tx.clone();d.connect_response(move|d,r|{if r==gtk::ResponseType::Ok{let _=tx.send(Event::Reset);}d.close();});d.show_all();});
                reset.set_halign(gtk::Align::End);
                reset.style_context().add_class("quiet");
                reset.style_context().add_class("destructive");
                page.pack_start(&reset, false, false, 0);
            }
        }
        page
    }
    fn shortcut(&self, parent: &gtk::Box, title: &str, toggle: bool, current: &str) {
        let entry = gtk::Entry::new();
        entry.set_text(&shortcut_text(current));
        entry.style_context().add_class("shortcut");
        entry.set_editable(false);
        entry.set_width_chars(18);
        row(
            parent,
            title,
            "Click, then press a shortcut; Escape cancels.",
            &entry,
        );
        let tx = self.tx.clone();
        entry.connect_focus_in_event(move |e, _| {
            e.set_text("Press shortcut…");
            e.style_context().add_class("recording");
            let _ = tx.send(Event::SuspendHotkeys(true));
            glib::Propagation::Proceed
        });
        let tx = self.tx.clone();
        entry.connect_button_press_event(move |e, event| {
            if event.button() == 1 {
                e.set_text("Press shortcut…");
                e.style_context().add_class("recording");
                let _ = tx.send(Event::SuspendHotkeys(true));
            }
            glib::Propagation::Proceed
        });
        let tx = self.tx.clone();
        let model = self.model.clone();
        entry.connect_focus_out_event(move |e, _| {
            e.style_context().remove_class("recording");
            let cfg = model.borrow();
            e.set_text(&shortcut_text(if toggle {
                &cfg.toggle
            } else {
                &cfg.clear
            }));
            let _ = tx.send(Event::SuspendHotkeys(false));
            glib::Propagation::Proceed
        });
        let m = self.model.clone();
        let tx = self.tx.clone();
        entry.connect_key_press_event(move |e, event| {
            let name = event
                .keyval()
                .name()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let strong = gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::MOD1_MASK
                | gdk::ModifierType::SUPER_MASK;
            if (name == "Tab" || name == "ISO_Left_Tab") && !event.state().intersects(strong) {
                return glib::Propagation::Proceed;
            }
            if !e.style_context().has_class("recording") {
                if name == "Return" || name == "space" {
                    e.set_text("Press shortcut…");
                    e.style_context().add_class("recording");
                    let _ = tx.send(Event::SuspendHotkeys(true));
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }
            if name == "Escape" {
                let model = m.borrow();
                e.set_text(&shortcut_text(if toggle {
                    &model.toggle
                } else {
                    &model.clear
                }));
                e.style_context().remove_class("recording");
                drop(model);
                let _ = tx.send(Event::SuspendHotkeys(false));
                return glib::Propagation::Stop;
            }
            let mods = event.state();
            let mut key = String::new();
            for (mask, label) in [
                (gdk::ModifierType::CONTROL_MASK, "Control+"),
                (gdk::ModifierType::MOD1_MASK, "Alt+"),
                (gdk::ModifierType::SUPER_MASK, "Super+"),
                (gdk::ModifierType::SHIFT_MASK, "Shift+"),
            ] {
                if mods.contains(mask) {
                    key.push_str(label);
                }
            }
            key.push_str(&if name.len() == 1 {
                name.to_ascii_lowercase()
            } else {
                name
            });
            if config::valid_shortcut(&key) {
                e.set_text(&shortcut_text(&key));
                e.style_context().remove_class("recording");
                change(&m, &tx, |c| {
                    if toggle {
                        c.toggle = key.clone()
                    } else {
                        c.clear = key.clone()
                    }
                });
                let _ = tx.send(Event::SuspendHotkeys(false));
            }
            glib::Propagation::Stop
        });
    }
    pub fn sync(&self, c: &Config) {
        *self.model.borrow_mut() = c.clone();
        let selected = c.selected_id();
        for (id, b) in &self.pointer_buttons {
            if *id == selected {
                b.style_context().add_class("selected");
            } else {
                b.style_context().remove_class("selected");
            }
        }
        if let Some(area) = &self.appearance {
            area.queue_draw();
        }
    }
    pub fn show_draft(&self, prepared: PreparedHand) {
        if let Ok(pix) =
            gtk::gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(prepared.png.clone()))
        {
            let head = pix.new_subpixbuf(112, 90, 800, 1000);
            self.preview.set_from_pixbuf(
                head.scale_simple(144, 180, gtk::gdk_pixbuf::InterpType::Bilinear)
                    .as_ref(),
            );
        }
        if self.name.text().is_empty() {
            self.name.set_text(&prepared.name);
        }
        *self.draft.borrow_mut() = Some(prepared);
        self.add.set_sensitive(true);
        self.message("Preview ready. Check the hand, choose a name, then Add hand.");
    }
    pub fn welcome(&self) {
        let d=gtk::MessageDialog::new(Some(&self.window),gtk::DialogFlags::MODAL,gtk::MessageType::Info,gtk::ButtonsType::None,"TinyHand is ready. Click to tap, hold and drag to draw. Hold your interaction key to click for real. Escape exits presentation mode.");
        d.set_title("Welcome to TinyHand");
        d.add_button("Settings", gtk::ResponseType::Cancel);
        d.add_button("Start", gtk::ResponseType::Accept);
        let tx = self.tx.clone();
        d.connect_response(move |d, r| {
            if r == gtk::ResponseType::Accept {
                let _ = tx.send(Event::Start);
            }
            d.close();
        });
        d.show_all();
    }
}

fn change(m: &Model, tx: &Sender<Event>, f: impl FnOnce(&mut Config)) {
    let cfg = {
        let mut c = m.borrow_mut();
        f(&mut c);
        c.clone()
    };
    let _ = tx.send(Event::Configure(cfg));
}
fn label(text: &str, muted: bool) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_line_wrap(true);
    if muted {
        l.style_context().add_class("muted");
    }
    l
}
fn card(parent: &gtk::Box, title: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 9);
    if !title.is_empty() {
        let heading = label(title, false);
        heading.style_context().add_class("section");
        section.pack_start(&heading, false, false, 0);
    }
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.style_context().add_class("card");
    section.pack_start(&body, false, false, 0);
    parent.pack_start(&section, false, false, 0);
    body
}
fn row(parent: &gtk::Box, title: &str, detail: &str, widget: &impl IsA<gtk::Widget>) {
    let widget: &gtk::Widget = widget.as_ref();
    if let Some(a) = widget.accessible() {
        a.set_name(title);
    }
    widget.set_valign(gtk::Align::Center);
    if !parent.children().is_empty() {
        let line = gtk::Separator::new(gtk::Orientation::Horizontal);
        line.style_context().add_class("row-divider");
        parent.pack_start(&line, false, false, 0);
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    row.style_context().add_class("setting-row");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_hexpand(true);
    let heading = label(title, false);
    heading.style_context().add_class("row-title");
    heading.set_width_chars(20);
    heading.set_max_width_chars(42);
    text.pack_start(&heading, false, false, 0);
    if !detail.is_empty() {
        let description = label(detail, true);
        description.set_width_chars(22);
        description.set_max_width_chars(43);
        text.pack_start(&description, false, false, 0);
    }
    row.pack_start(&text, true, true, 0);
    row.pack_end(widget, false, false, 0);
    parent.pack_start(&row, false, false, 0);
}
fn switch(
    parent: &gtk::Box,
    title: &str,
    detail: &str,
    value: bool,
    m: &Model,
    tx: &Sender<Event>,
    f: impl Fn(&mut Config, bool) + 'static,
) {
    let w = gtk::Switch::new();
    w.set_active(value);
    w.set_valign(gtk::Align::Center);
    row(parent, title, detail, &w);
    let m = m.clone();
    let tx = tx.clone();
    w.connect_active_notify(move |w| change(&m, &tx, |c| f(c, w.is_active())));
}
fn slider(
    parent: &gtk::Box,
    title: &str,
    value: f64,
    range: (f64, f64, f64),
    m: &Model,
    tx: &Sender<Event>,
    f: impl Fn(&mut Config, f64) + 'static,
) {
    let (min, max, step) = range;
    let w = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, step);
    w.set_width_request(160);
    w.set_draw_value(false);
    w.set_value(value);
    if let Some(a) = w.accessible() {
        a.set_name(title);
    }
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    controls.pack_start(&w, false, false, 0);
    let output = label(&slider_value(title, value), false);
    output.set_xalign(0.5);
    output.set_width_chars(5);
    output.style_context().add_class("value-pill");
    controls.pack_start(&output, false, false, 0);
    row(parent, title, "", &controls);
    let m = m.clone();
    let tx = tx.clone();
    let title = title.to_owned();
    w.connect_value_changed(move |w| {
        output.set_text(&slider_value(&title, w.value()));
        change(&m, &tx, |c| f(c, w.value()));
    });
}
fn slider_value(title: &str, value: f64) -> String {
    match title {
        "Size" | "Opacity" | "Volume" => format!("{:.0}%", value * 100.0),
        "Tilt" => format!("{:+.0}°", value.to_degrees()),
        "Thickness" => format!("{value:.0} px"),
        _ => format!("{value:.2}"),
    }
}

fn thumbnail(id: &str, cache: &Thumbnails) -> Option<gtk::gdk_pixbuf::Pixbuf> {
    if id == "laser" {
        return None;
    }
    if let Some(image) = cache.borrow().get(id) {
        return Some(image.clone());
    }
    let path = if id.starts_with("custom-") {
        Library::path(id)
    } else {
        crate::render::asset_dir()
            .ok()?
            .join(format!("hand_{id}.png"))
    };
    let image = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 128, 192, true).ok()?;
    let (w, h) = (image.width(), (image.height() as f64 * 0.49) as i32);
    let stride = image.rowstride() as usize;
    let channels = image.n_channels() as usize;
    let bytes = image.read_pixel_bytes();
    let bytes = bytes.as_ref();
    let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if !image.has_alpha() || bytes[y as usize * stride + x as usize * channels + 3] > 16 {
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
                any = true;
            }
        }
    }
    let cropped = if any {
        image.new_subpixbuf(left, top, right - left + 1, bottom - top + 1)
    } else {
        image.new_subpixbuf(0, 0, w, h.max(1))
    };
    cache.borrow_mut().insert(id.to_owned(), cropped.clone());
    Some(cropped)
}
fn pointer_button(id: &str, library: &Library, cache: &Thumbnails) -> gtk::Button {
    pointer_tile(id, library, cache, false, false)
}
fn pointer_tile(
    id: &str,
    library: &Library,
    cache: &Thumbnails,
    gallery: bool,
    pinned: bool,
) -> gtk::Button {
    let b = gtk::Button::new();
    b.style_context().add_class("pointer-tile");
    if pinned {
        b.style_context().add_class("pinned");
    }
    if let Some(a) = b.accessible() {
        a.set_name(id);
    }
    let name = library.hand(id).map(|h| h.name.clone()).unwrap_or_else(|| {
        let mut c = id.chars();
        c.next()
            .map(|first| first.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    });
    b.set_tooltip_text(Some(&name));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 5);
    let overlay = gtk::Overlay::new();
    let art = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let size = if gallery { 46 } else { 38 };
    art.set_size_request(size + 4, size + 4);
    if id == "laser" {
        let dot = gtk::DrawingArea::new();
        dot.set_size_request(size, size);
        dot.connect_draw(|w, c| {
            let x = w.allocated_width() as f64 / 2.0;
            let y = w.allocated_height() as f64 / 2.0;
            let (r, g, b) = crate::colors::RED;
            c.set_source_rgba(r, g, b, 0.14);
            c.arc(x, y, 11.0, 0.0, std::f64::consts::TAU);
            let _ = c.fill();
            c.set_source_rgba(r, g, b, 1.0);
            c.arc(x, y, 6.0, 0.0, std::f64::consts::TAU);
            let _ = c.fill();
            c.set_source_rgba(1.0, 0.86, 0.82, 0.95);
            c.arc(x - 1.0, y - 1.0, 2.2, 0.0, std::f64::consts::TAU);
            let _ = c.fill();
            glib::Propagation::Proceed
        });
        art.pack_start(&dot, true, true, 0);
    } else if let Some(pix) = thumbnail(id, cache) {
        let scale = (size as f64 / pix.width() as f64).min(size as f64 / pix.height() as f64);
        let image = gtk::Image::from_pixbuf(
            pix.scale_simple(
                (pix.width() as f64 * scale).round() as i32,
                (pix.height() as f64 * scale).round() as i32,
                gtk::gdk_pixbuf::InterpType::Bilinear,
            )
            .as_ref(),
        );
        art.pack_start(&image, true, true, 0);
    }
    overlay.add(&art);
    if gallery && pinned {
        let badge = gtk::Image::from_icon_name(Some("emblem-ok-symbolic"), gtk::IconSize::Menu);
        badge.set_pixel_size(10);
        badge.set_halign(gtk::Align::End);
        badge.set_valign(gtk::Align::Start);
        badge.style_context().add_class("pin-badge");
        overlay.add_overlay(&badge);
        overlay.set_overlay_pass_through(&badge, true);
    }
    content.pack_start(&overlay, false, false, 0);
    if gallery {
        let caption = label(&name, false);
        caption.set_xalign(0.5);
        caption.set_line_wrap(false);
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        caption.set_max_width_chars(10);
        caption.style_context().add_class("pointer-name");
        content.pack_start(&caption, false, false, 0);
        b.set_size_request(72, 80);
    }
    b.add(&content);
    b
}
fn appearance_preview(model: Model, cache: Thumbnails) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_size_request(-1, 148);
    if let Some(a) = area.accessible() {
        a.set_name("Pointer preview");
    }
    area.connect_draw(move |w, c| {
        let cfg = model.borrow();
        let hand = thumbnail(&cfg.selected_id(), &cache);
        let (width, height) = (w.allocated_width() as f64, w.allocated_height() as f64);
        gtk::render_background(&w.style_context(), c, 0.0, 0.0, width, height);
        gtk::render_frame(&w.style_context(), c, 0.0, 0.0, width, height);
        c.set_source_rgba(1.0, 1.0, 1.0, 0.045);
        for y in (12..height as i32).step_by(18) {
            for x in (12..width as i32).step_by(18) {
                c.arc(x as f64, y as f64, 0.8, 0.0, std::f64::consts::TAU);
                let _ = c.fill();
            }
        }
        let _ = c.save();
        c.translate(width * 0.43, 38.0);
        if cfg.style == 36 && !cfg.custom_enabled {
            let (r, g, b) = crate::colors::RED;
            let radius = 8.0 * cfg.size / 0.13;
            c.set_source_rgba(r, g, b, 0.12);
            c.arc(0.0, 16.0, radius * 2.5, 0.0, std::f64::consts::TAU);
            let _ = c.fill();
            c.set_source_rgba(r, g, b, cfg.opacity);
            c.arc(0.0, 16.0, radius, 0.0, std::f64::consts::TAU);
            let _ = c.fill();
        } else if let Some(hand) = &hand {
            c.rotate(-0.30 + cfg.angle);
            let scale = 66.0 * cfg.size / 0.13 / hand.height() as f64;
            c.scale(scale, scale);
            let x = -(hand.width() as f64) / 2.0;
            let end = height / scale + 80.0;
            if cfg.shadow {
                let _ = c.save();
                c.translate(7.0, 9.0);
                c.set_line_width(6.0);
                c.set_source_rgba(0.0, 0.0, 0.0, 0.18 * cfg.opacity);
                c.move_to(0.0, hand.height() as f64 - 4.0);
                c.line_to(0.0, end);
                let _ = c.stroke();
                c.set_source_pixbuf(hand, x, 0.0);
                let mask = c.source();
                c.set_source_rgba(0.0, 0.0, 0.0, 0.18 * cfg.opacity);
                let _ = c.mask(&mask);
                let _ = c.restore();
            }
            c.set_line_cap(gtk::cairo::LineCap::Round);
            for (offset, line, colour) in [(0.0, 5.5, 0.18), (-0.7, 3.2, 0.65), (-1.1, 1.0, 0.9)] {
                c.set_line_width(line);
                c.set_source_rgba(colour, colour, colour, cfg.opacity);
                c.move_to(offset, hand.height() as f64 - 5.0);
                c.line_to(offset, end);
                let _ = c.stroke();
            }
            c.set_source_pixbuf(hand, x, 0.0);
            let _ = c.paint_with_alpha(cfg.opacity);
        }
        let _ = c.restore();
        c.set_source_rgba(0.65, 0.65, 0.71, 1.0);
        c.select_font_face(
            "Sans",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Normal,
        );
        c.set_font_size(10.0);
        c.move_to(width - 82.0, height - 16.0);
        let _ = c.show_text("LIVE PREVIEW");
        glib::Propagation::Proceed
    });
    area
}
fn shortcut_text(value: &str) -> String {
    value
        .split('+')
        .map(|p| match p {
            "Control" => "Ctrl".into(),
            "Return" => "Enter".into(),
            "space" => "Space".into(),
            _ if p.len() == 1 => p.to_uppercase(),
            _ => p.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}
