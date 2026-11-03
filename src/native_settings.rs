//! Native, keyboard-accessible GTK settings. No presentation grab is held here.
use crate::{
    config::{self, Config, STYLES},
    desktop::{self, Event},
    hands::{self, Library, PreparedHand},
};
use gtk::{gdk, glib, prelude::*};
use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

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
}
type Model = Rc<RefCell<Config>>;

impl SettingsWindow {
    pub fn new(tx: Sender<Event>) -> Self {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("TinyHand Settings");
        window.set_position(gtk::WindowPosition::Center);
        window.set_default_size(780, 560);
        window.set_size_request(650, 480);
        let css = gtk::CssProvider::new();
        let _=css.load_from_data(b"window.tinyhand {background:#1b1b1d;color:#eee;} .sidebar{background:#3c4042;padding:36px 10px 12px;} .sidebar button{background:transparent;border:0;color:#eee;box-shadow:none;text-shadow:none;padding:7px;} .sidebar button:checked{background:#565b5e;} .page{padding:28px;} .page label{color:#eee;} .page button label{color:inherit;} button{background-image:none;background-color:#343438;color:#eee;border-color:#505055;text-shadow:none;} button:hover{background-color:#45454b;} button:checked{background-color:#374c70;border-color:#6ca7ff;} button.swatch{min-width:22px;min-height:22px;padding:4px;} .card{background:#262628;border-radius:12px;padding:8px 14px;} .muted{color:#a0a1a6;font-size:11px;} .heading{font-size:22px;font-weight:bold;} .section{font-size:12px;font-weight:bold;color:#aaa;} button.tile{background:#27272a;border-radius:8px;padding:5px;} button.tile:checked{background:#233c5d;border-color:#378df2;} .preview{background:#dcc7ae;border-radius:10px;padding:8px;} .footer{padding:10px;} entry{min-height:26px;} ");
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
        for c in self.body.children() {
            self.body.remove(&c);
        }
        *self.model.borrow_mut() = config.clone();
        let horizontal = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        self.body.pack_start(&horizontal, true, true, 0);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 3);
        sidebar.set_width_request(180);
        sidebar.style_context().add_class("sidebar");
        horizontal.pack_start(&sidebar, false, false, 0);
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
            self.stack.add_titled(&scrolled, title, title);
            if self.tab == *title {
                b.set_active(true);
            }
        }
        let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar.pack_start(&spacer, true, true, 0);
        sidebar.pack_end(
            &gtk::Label::new(Some("TinyHand for Linux")),
            false,
            false,
            0,
        );
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.style_context().add_class("footer");
        self.status = gtk::Label::new(None);
        self.status.set_xalign(0.0);
        self.status.set_line_wrap(true);
        self.status.style_context().add_class("muted");
        footer.pack_start(&self.status, true, true, 0);
        let start = gtk::Button::with_label("Start pointer");
        let tx = self.tx.clone();
        start.connect_clicked(move |_| {
            let _ = tx.send(Event::Start);
        });
        footer.pack_end(&start, false, false, 0);
        let quit = gtk::Button::with_label("Quit TinyHand");
        let tx = self.tx.clone();
        quit.connect_clicked(move |_| {
            let _ = tx.send(Event::Quit);
        });
        footer.pack_end(&quit, false, false, 0);
        self.body.pack_end(&footer, false, false, 0);
        self.body.show_all();
        self.stack.set_visible_child_name(&self.tab);
        self.window.present();
        gtk::prelude::GtkWindowExt::set_focus(&self.window, Some(&start));
    }
    fn page(&mut self, index: usize, cfg: &Config, library: &Library) -> gtk::Box {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
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
        page.pack_start(&title, false, false, 0);
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
                    let button = pointer_button(id, library);
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
                    let remove = gtk::Button::with_label("×");
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
                flow.set_max_children_per_line(8);
                flow.set_min_children_per_line(4);
                flow.set_column_spacing(4);
                flow.set_row_spacing(4);
                for id in STYLES
                    .iter()
                    .map(|s| s.to_string())
                    .chain(library.hands.iter().map(|h| h.id.clone()))
                {
                    let b = pointer_button(&id, library);
                    if let Some(a) = b.accessible() {
                        a.set_name(&format!("Pin {id}"));
                    }
                    if cfg.pinned.contains(&id) {
                        b.style_context().add_class("suggested-action");
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
                            let swap = pointer_button(old, library);
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
                page.pack_start(&flow, false, false, 0);
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
                page.pack_start(&reset, false, false, 0);
            }
            2 => {
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
                for (kind, title) in crate::settings::SOUNDS.iter().enumerate() {
                    let b = gtk::Button::with_label(title);
                    b.set_size_request(100, 65);
                    b.style_context().add_class("tile");
                    if (kind == 0 && !cfg.sound) || (cfg.sound && kind == cfg.sound_kind) {
                        b.style_context().add_class("suggested-action");
                    }
                    let m = self.model.clone();
                    let tx = self.tx.clone();
                    b.connect_clicked(move |_| {
                        change(&m, &tx, |c| {
                            c.sound = kind > 0;
                            if kind > 0 {
                                c.sound_kind = kind;
                            }
                        });
                        let _ = tx.send(Event::Message(format!("preview-sound:{kind}")));
                    });
                    flow.insert(&b, -1);
                }
                group.pack_start(&flow, false, false, 0);
                page.pack_start(
                    &label(
                        "Click a sound to hear it. It plays each time the hand taps.",
                        true,
                    ),
                    false,
                    false,
                    0,
                );
                let other = card(&page, "");
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
                page.pack_start(
                    &label("A tiny hand for your screen. Free and open source.", false),
                    false,
                    false,
                    0,
                );
                page.pack_start(
                    &label(concat!("Version ", env!("CARGO_PKG_VERSION")), true),
                    false,
                    false,
                    0,
                );
                for (title, url) in [
                    ("Website", "https://tinyhand.app"),
                    ("Original source", "https://github.com/nilbuild/tinyhand"),
                    ("Made by @nilbuild", "https://x.com/nilbuild"),
                    (
                        "MIT license",
                        "https://github.com/nilbuild/tinyhand/blob/main/LICENSE",
                    ),
                ] {
                    let b = gtk::LinkButton::with_label(url, title);
                    page.pack_start(&b, false, false, 0);
                }

                let reset = gtk::Button::with_label("Reset all settings…");
                let parent = self.window.clone();
                let tx = self.tx.clone();
                reset.connect_clicked(move |_|{let d=gtk::MessageDialog::new(Some(&parent),gtk::DialogFlags::MODAL,gtk::MessageType::Question,gtk::ButtonsType::OkCancel,"Reset shortcuts, pointers, appearance, drawing and sound? Custom hands and startup preferences are kept.");let tx=tx.clone();d.connect_response(move|d,r|{if r==gtk::ResponseType::Ok{let _=tx.send(Event::Reset);}d.close();});d.show_all();});
                page.pack_start(&reset, false, false, 0);
            }
        }
        page
    }
    fn shortcut(&self, parent: &gtk::Box, title: &str, toggle: bool, current: &str) {
        let entry = gtk::Entry::new();
        entry.set_text(current);
        entry.set_editable(false);
        entry.set_width_chars(20);
        row(
            parent,
            title,
            "Click, then press a shortcut; Escape cancels.",
            &entry,
        );
        let tx = self.tx.clone();
        entry.connect_focus_in_event(move |e, _| {
            e.set_text("Press keys…");
            let _ = tx.send(Event::SuspendHotkeys(true));
            glib::Propagation::Proceed
        });
        let tx = self.tx.clone();
        entry.connect_focus_out_event(move |_, _| {
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
            if name == "Escape" {
                let model = m.borrow();
                e.set_text(if toggle { &model.toggle } else { &model.clear });
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
                e.set_text(&key);
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
    if !title.is_empty() {
        let h = label(title, false);
        h.style_context().add_class("section");
        parent.pack_start(&h, false, false, 0);
    }
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.style_context().add_class("card");
    parent.pack_start(&b, false, false, 0);
    b
}
fn row(parent: &gtk::Box, title: &str, detail: &str, widget: &impl IsA<gtk::Widget>) {
    let widget: &gtk::Widget = widget.as_ref();
    if let Some(a) = widget.accessible() {
        a.set_name(title);
    }
    widget.set_valign(gtk::Align::Center);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.pack_start(&label(title, false), false, false, 0);
    if !detail.is_empty() {
        text.pack_start(&label(detail, true), false, false, 0);
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
    w.set_width_request(180);
    w.set_digits(if step < 1.0 { 2 } else { 0 });
    w.set_value(value);
    row(parent, title, "", &w);
    let m = m.clone();
    let tx = tx.clone();
    w.connect_value_changed(move |w| change(&m, &tx, |c| f(c, w.value())));
}
fn pointer_button(id: &str, library: &Library) -> gtk::Button {
    let b = gtk::Button::new();
    if let Some(a) = b.accessible() {
        a.set_name(id);
    }
    b.style_context().add_class("tile");
    b.set_tooltip_text(Some(id));
    if id == "laser" {
        b.set_label("🔴");
        b.set_size_request(44, 48);
        return b;
    }
    let path = if id.starts_with("custom-") {
        Library::path(id)
    } else {
        crate::render::asset_dir()
            .unwrap_or_default()
            .join(format!("hand_{id}.png"))
    };
    if let Ok(pix) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 80, 120, true) {
        let h = (pix.height() as f64 * 0.49) as i32;
        let head = pix.new_subpixbuf(0, 0, pix.width(), h.max(1));
        let image = gtk::Image::from_pixbuf(
            head.scale_simple(40, 40, gtk::gdk_pixbuf::InterpType::Bilinear)
                .as_ref(),
        );
        b.add(&image);
    }
    if let Some(hand) = library.hand(id) {
        b.set_tooltip_text(Some(&hand.name));
    }
    b
}
