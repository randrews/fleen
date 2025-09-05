use eframe::egui::{Align, Button, Color32, InnerResponse, IntoAtoms, Layout, Response, RichText, Ui, Widget};

pub trait UiExtensions {
    /// A widget that fills the width of its container. Good for buttons.
    fn add_fill_width(&mut self, widget: impl Widget) -> Response;

    /// A sub-Ui with a specific width and 100% height
    fn column<R>(&mut self, width: f32, f: impl FnOnce(&mut Self)-> R) -> InnerResponse<R>;
}

impl UiExtensions for Ui {
    fn add_fill_width(&mut self, widget: impl Widget) -> Response {
        self.add_sized((self.available_width(), 20.0), widget)
    }

    fn column<R>(&mut self, width: f32, f: impl FnOnce(&mut Self) -> R) -> InnerResponse<R> {
        let third = (width, self.available_height()).into();
        self.allocate_ui_with_layout(third, Layout::top_down(Align::Max), f)
    }
}

pub trait ButtonExtensions<'a> {
    fn red(atoms: impl IntoAtoms<'a>) -> Self;
    fn green(text: impl Into<String>) -> Self;
    fn blue(text: impl Into<String>) -> Self;
}

impl<'a> ButtonExtensions<'a> for Button<'a> {
    fn red(atoms: impl IntoAtoms<'a>) -> Self {
        Self::new(atoms).fill(Color32::DARK_RED)
    }

    fn green(text: impl Into<String>) -> Self {
        Self::new(RichText::new(text).color(Color32::WHITE)).fill(Color32::DARK_GREEN)
    }

    fn blue(text: impl Into<String>) -> Self {
        Self::new(RichText::new(text).color(Color32::WHITE)).fill(Color32::DARK_BLUE)
    }
}