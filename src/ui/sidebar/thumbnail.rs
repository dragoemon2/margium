use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Image, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, 
    SelectionMode, Align, PolicyType, gdk
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::{self, PdfEngine};

pub struct ThumbnailResult {
    pub page_index: i32,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub pixels: Vec<u8>,
}

pub struct ThumbnailWidget {
    pub list: ListBox,
    pub scroll: ScrolledWindow,
}

impl ThumbnailWidget {
    pub fn new(
        engine: Rc<RefCell<PdfEngine>>,
        drawing_area: &gtk4::DrawingArea,
    ) -> Self {
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::Single);

        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .child(&list)
            .build();

        Self { list, scroll }
    }

    pub fn prepare_empty_thumbnails(&self, engine: &PdfEngine) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        for i in 0..engine.get_total_pages() {
            let row = ListBoxRow::new();
            let vbox = GtkBox::new(Orientation::Vertical, 5);
            vbox.set_margin_top(10);
            vbox.set_margin_bottom(10);
            vbox.set_halign(Align::Center);
            
            let image_widget = Image::new();
            image_widget.set_pixel_size(150);
            image_widget.set_icon_name(Some("image-loading-symbolic"));
            
            let label_text = engine.get_page_label(i)
                .unwrap_or_else(|| format!("{}", i + 1));
            let label = Label::new(Some(&label_text));
            
            vbox.append(&image_widget);
            vbox.append(&label);
            row.set_child(Some(&vbox));
            
            self.list.append(&row);
        }
    }

    pub fn set_thumbnail_image(&self, page_index: i32, texture: &gdk::Texture) {
        if let Some(row) = self.list.row_at_index(page_index) {
            if let Some(vbox) = row.child().and_then(|c| c.downcast::<GtkBox>().ok()) {
                if let Some(img) = vbox.first_child().and_then(|c| c.downcast::<Image>().ok()) {
                    img.set_paintable(Some(texture));
                }
            }
        }
    }

    pub fn scroll_to_thumbnail(&self, page_num: i32) {
        if let Some(row) = self.list.row_at_index(page_num) {
            self.list.select_row(Some(&row));
            // 行が見える位置までスクロール
            row.grab_focus();
        }
    }
}