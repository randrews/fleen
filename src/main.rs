mod fleen_app;
mod renderer;
mod server;
mod ui_ext;
mod utils;
mod site_ui;

use eframe::egui::{Button, Context, RichText};
use eframe::{egui, Frame};
use site_ui::SiteUi;
use crate::fleen_app::{FleenError, Site};
use crate::ui_ext::{ButtonExtensions, UiExtensions};

#[tokio::main]
async fn main() {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options.viewport.with_icon(
        eframe::icon_data::from_png_bytes(include_bytes!("../icon/128x128@2x.png"))
            .expect("Failed to load icon")
    );
    eframe::run_native("Fleen", native_options, Box::new(|_cc| {
        Ok(Box::new(FleenUi(None, None)))
    })).expect("Error running application");
}

struct FleenUi(Option<SiteUi>, Option<FleenError>);

fn site_chooser(ctx: &Context, error: &Option<FleenError>) -> Result<Option<Site>, FleenError> {
    if let Some(err) = error {
        let message = format!("{}", err);
        egui::Window::new("Error").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(message);
        });
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        let title = egui::Label::new(RichText::new("Select a site to manage").size(20.0));
        ui.vertical_centered(|ui| ui.add(title));

        if ui.add_fill_width(Button::blue("Open site...")).clicked() && let Some(path) = rfd::FileDialog::new().pick_folder() {
            match Site::open(&path) {
                Ok(site) => { return Ok(Some(site)) }
                Err(err) => { return Err(err) }
            }
        }

        if ui.add_fill_width(Button::green("New site...")).clicked() && let Some(path) = rfd::FileDialog::new().pick_folder() {
            match Site::create(&path) {
                Ok(site) => { return Ok(Some(site)) }
                Err(err) => { return Err(err) }
            }
        }

        Ok(None)
    }).inner
}

impl eframe::App for FleenUi {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        match &mut self.0 {
            None => {
                match site_chooser(ctx, &self.1) {
                    Ok(Some(site)) => { self.0 = Some(SiteUi::from(site))}
                    Err(e) => { self.1 = Some(e) }
                    _ => {}
                }
            }
            Some(site_ui) => site_ui.display(ctx)
        }
    }
}
