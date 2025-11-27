use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use eframe::egui;
use eframe::egui::{Button, Context, Id};
use egui_ltreeview::Action;
use tokio::task::JoinHandle;
use crate::utils;
use crate::fleen_app::{FileType, FleenError, Site, SiteActions, TreeEntry};
use crate::server::start_server;
use crate::ui_ext::{ButtonExtensions, UiExtensions};
use crate::utils::{open_filename, open_server};

pub struct SiteUi {
    site: Arc<Site>,
    error: Option<FleenError>,
    message: Option<String>,
    selected_file: Option<String>,
    dialog_mode: Option<DialogMode>,
    server_handle: Option<JoinHandle<()>>,
    server_port: String,
    deploy_response: Arc<Mutex<Option<Result<String, FleenError>>>>,
    deploying: bool,
    image_message: Option<TempMessage>,
}

impl From<Site> for SiteUi {
    fn from(value: Site) -> Self {
        Self {
            site: Arc::new(value),
            error: None,
            message: None,
            selected_file: None,
            dialog_mode: None,
            server_handle: None,
            server_port: "3000".to_string(),
            deploy_response: Arc::new(Mutex::new(None)),
            deploying: false,
            image_message: None,
        }
    }
}

impl SiteUi {
    pub fn display(&mut self, ctx: &Context) {
        self.check_deploy_status(ctx);
        self.error_dialog(ctx);
        self.message_dialog(ctx);
        self.temp_message();

        let mut just_clicked = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            let width = ui.available_width() / 3.0 - 5.0;
            let height = ui.available_height() - 120.0;
            ui.horizontal(|ui| {
                ui.column(width, |ui| {
                    egui::ScrollArea::new([true, true])
                        .auto_shrink([false, false])
                        .min_scrolled_height(height)
                        .show(ui, |ui| self.tree_view(ui));
                    just_clicked = self.tree_buttons(ui);
                });
                ui.column(width, |ui| self.server_controls(ui));
                ui.column(width, |ui| {
                    ui.add_enabled_ui(!self.deploying, |ui| {
                        let label = if self.deploying {
                            "Deploying..."
                        } else {
                            "Build and Deploy"
                        };
                        if ui.add_fill_width(Button::green(label)).clicked() {
                            self.build_and_deploy();
                        }
                    });

                    if ui.add_fill_width(Button::blue("Build site...")).clicked() &&
                        let Some(path) = rfd::FileDialog::new().pick_folder() {
                        match self.site.build_site(&path) {
                            Ok(()) => { self.message = Some("Site built successfully".to_string()) }
                            Err(err) => { self.error = Some(err) }
                        }
                    }
                })
            });
        });

        match self.dialog_mode {
            Some(DialogMode::NewFile(_)) => self.new_file_dialog(ctx, just_clicked),
            Some(DialogMode::ConfirmDelete(_)) => self.confirm_delete_dialog(ctx),
            Some(DialogMode::RenameFile(_)) => self.rename_dialog(ctx, just_clicked),
            None => {}
        }
    }

    fn check_deploy_status(&mut self, ctx: &Context) {
        if self.deploying && let Ok(mut m) = self.deploy_response.lock() {
            if let Some(result) = m.take() {
                match result {
                    Err(e) => { self.error = Some(e) }
                    Ok(s) => { self.message = Some(s) }
                }
                self.deploying = false;
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// If there's an error dialog, display it
    fn error_dialog(&mut self, ctx: &Context) {
        if let Some(err) = &self.error {
            let message = format!("{}", err);
            egui::Window::new("Error").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(message);
                if ui.button("I see").clicked() {
                    self.error = None
                }
            });
        }
    }

    /// If there's an informational dialog, display it
    fn message_dialog(&mut self, ctx: &Context) {
        if let Some(message) = &self.message {
            let message = message.clone();
            egui::Window::new("FYI").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(message);
                if ui.button("Thanks!").clicked() {
                    self.message = None
                }
            });
        }
    }

    /// A temporary message (replaces the label on the image button)
    fn temp_message(&mut self) {
        if let Some(TempMessage { created, .. }) = &self.image_message
            && *created < Instant::now() - Duration::from_secs(2) {
            self.image_message = None
        }
    }

    fn build_and_deploy(&mut self) {
        self.deploying = true;
        let mutex = self.deploy_response.clone();
        let site = self.site.clone();
        tokio::spawn(async move {
            let result = site.build_and_deploy().await;
            if let Ok(mut m) = mutex.lock() {
                *m = Some(result);
            }
        });
    }

    fn tree_view(&mut self, ui: &mut egui::Ui) {
        let tv = egui_ltreeview::TreeView::new(Id::from("tree"))
            .allow_multi_selection(false)
            .allow_drag_and_drop(false);
        let (_, actions) = tv.show(ui, |builder| {
            for entry in self.site.tree.iter() {
                match entry {
                    TreeEntry::File(p) => builder.leaf(utils::id_for_path(p), utils::label_for_path(p)),
                    TreeEntry::Dir(p) => { builder.dir(utils::id_for_path(p), utils::label_for_path(p)); },
                    TreeEntry::CloseDir => builder.close_dir()
                }
            }
        });

        for action in actions {
            match action {
                Action::SetSelected(files) => {
                    self.selected_file = files.first().cloned()
                }
                Action::Activate(activate) => {
                    for fname in activate.selected {
                        if let Err(e) = open_filename(&fname) { self.error = Some(e) }
                    }
                }
                _ => {}
            }
        }
    }

    fn tree_buttons(&mut self, ui: &mut egui::Ui) -> bool {
        let mut just_clicked = false;

        if ui.add_fill_width(egui::Button::new("Open")).clicked() &&
            let Some(fname) = &self.selected_file &&
            let Err(e) = open_filename(fname) {
            self.error = Some(e)
        }

        let new_btn = Button::green("New page");
        if ui.add_fill_width(new_btn).clicked() {
            just_clicked = true;
            self.dialog_mode = Some(DialogMode::NewFile(String::new()));
        }

        let rename_btn = Button::new("Rename");
        let delete_btn = Button::red("Delete");
        if self.root_selected() || self.selected_file.is_none() {
            ui.add_enabled_ui(false, |ui| {
                ui.add_fill_width(rename_btn);
                ui.add_fill_width(delete_btn);
            });
        } else if let Some(selected) = &self.selected_file {
            if ui.add_fill_width(rename_btn).clicked() {
                self.dialog_mode = Some(DialogMode::RenameFile(utils::label_for_path(&PathBuf::from(&selected))));
                just_clicked = true;
            }
            if ui.add_fill_width(delete_btn).clicked() {
                self.dialog_mode = Some(DialogMode::ConfirmDelete(selected.clone()));
            }
        }

        ui.add_enabled_ui(self.site.image_dir_exists() && self.image_message.is_none(), |ui| {
            let label = match &self.image_message {
                Some(TempMessage { message, .. }) => message.as_str(),
                _ => "Image from clipboard"
            };

            if ui.add_fill_width(Button::blue(label)).clicked() {
                self.image_message = match self.site.paste_image() {
                    Ok(new_site) => {
                        self.site = Arc::new(new_site);
                        Some(TempMessage { message: "Image saved!".to_string(), created: Instant::now() })
                    },
                    Err(e) => Some(TempMessage { message: e.to_string(), created: Instant::now() })
                }
            }
        });
        just_clicked
    }

    fn server_controls(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.label("Port");
            let open_button = Button::new(format!("Open http://localhost:{}", self.server_port));
            let port_editor = egui::TextEdit::singleline(&mut self.server_port);

            if let Some(join_handle) = &self.server_handle {
                ui.add_enabled_ui(false, |ui| ui.add_fill_width(port_editor));
                let stop_btn = Button::red("Stop server");
                if ui.add_fill_width(stop_btn).clicked() {
                    join_handle.abort();
                    self.server_handle = None;
                }
                if ui.add_fill_width(open_button).clicked() {
                    open_server(self.server_port.as_str());
                }
            } else {
                ui.add(port_editor);
                let start_btn = Button::green("Start server");
                if let Ok(port_num) = self.server_port.parse::<u32>() {
                    if ui.add_fill_width(start_btn).clicked() {
                        let path = self.site.root.to_path_buf();
                        self.server_handle = Some(tokio::spawn(start_server(path, port_num)))
                    }
                } else {
                    ui.add_enabled_ui(false, |ui| ui.add_fill_width(start_btn));
                }
                ui.add_enabled_ui(false, |ui| ui.add_fill_width(open_button));
            }
        });
    }

    fn new_file_dialog(&mut self, ctx: &Context, just_clicked: bool) {
        egui::Window::new("New page").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("Name");
            // If we press enter on the text field, it emits lost_focus. We can't just create the file based
            // on that, because it's not the only thing that causes it, though. So instead we'll just focus the
            // create file button
            //let fname = match &mut self.dialog_mode { Some(DialogMode::NewFile(s)) => s, _ => unreachable!() };
            let Some(DialogMode::NewFile(fname)) = &mut self.dialog_mode else { unreachable!() };
            let name_field = egui::TextEdit::singleline(fname);
            let resp = ui.add(name_field);
            let enter_key = resp.lost_focus();

            // We want to focus the text field if we just opened the dialog, but egui doesn't currently
            // offer a way to do that. However, we can pass in a bool for if the button that opens the
            // dialog was just clicked, and use that to tell whether we should grab focus. It's only true
            // the exact frame that the dialog was opened, which is what we're looking for.
            if just_clicked { resp.request_focus() }

            ui.horizontal(|ui| {
                let mut make_thing = |file_type: FileType| {
                    let Some(DialogMode::NewFile(fname)) = &self.dialog_mode else { unreachable!() };
                    let r =
                        self.site.create_page(file_type,
                            fname,
                            self.selected_file.as_ref());
                    match r {
                        Ok(new_site) => {
                            self.site = Arc::new(new_site);
                            self.dialog_mode = None;
                        }
                        Err(e) => { self.error = Some(e) }
                    }
                };

                let btn = ui.button("New file");
                if enter_key { btn.request_focus() }
                if btn.clicked() { make_thing(FileType::File) }
                if ui.button("New directory").clicked() { make_thing(FileType::Dir) }
                if ui.button("Cancel").clicked() {
                    self.dialog_mode = None
                }
            })
        });
    }

    fn rename_dialog(&mut self, ctx: &Context, just_clicked: bool) {
        egui::Window::new("Rename").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("New name");
            let Some(DialogMode::RenameFile(fname)) = &mut self.dialog_mode else { unreachable!() };
            let name_field = egui::TextEdit::singleline(fname);
            let resp = ui.add(name_field);
            let enter_key = resp.lost_focus();
            if just_clicked { resp.request_focus() } // See new_file_dialog

            ui.horizontal(|ui| {
                let btn = ui.button("Rename");
                if enter_key { btn.request_focus() }
                if btn.clicked() {
                    let Some(DialogMode::RenameFile(fname)) = &self.dialog_mode else { unreachable!() };
                    match self.site.rename_page(self.selected_file.as_ref().unwrap(), fname) {
                        Ok(new_site) => {
                            self.site = Arc::new(new_site);
                            self.dialog_mode = None; // Close the dialog, we're done
                        }
                        Err(e) => { self.error = Some(e) }
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.dialog_mode = None
                }
            })
        });
    }

    fn confirm_delete_dialog(&mut self, ctx: &Context) {
        let (mut del, mut cancel) = (false, false);
        let Some(DialogMode::ConfirmDelete(fname)) = &self.dialog_mode else { unreachable!() };
        egui::Window::new("Are you sure?").collapsible(false).resizable(false).show(ctx, |ui| {
            let file = utils::label_for_path(&PathBuf::from(fname));
            ui.heading(format!("Really delete {}?", file));
            ui.horizontal(|ui| {
                let btn = Button::red("Yep, I'm sure");
                del = ui.add(btn).clicked();
                cancel = ui.button("Cancel").clicked();
            })
        });

        if del {
            match self.site.delete_page(fname) {
                Ok(new_site) => { self.site = Arc::new(new_site) }
                Err(e) => { self.error = Some(e) }
            }
            self.dialog_mode = None;
            self.selected_file = None;
        } else if cancel {
            self.dialog_mode = None
        }
    }

    fn root_selected(&self) -> bool {
        if let Some(path) = &self.selected_file {
            *path == self.site.root
        } else {
            false
        }
    }
}

enum DialogMode {
    NewFile(String),
    ConfirmDelete(String),
    RenameFile(String)
}

struct TempMessage {
    created: Instant,
    message: String
}