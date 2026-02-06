use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Label, Orientation, Stack, ScrolledWindow, 
    ListBox, ListBoxRow, SearchEntry, Image, SelectionMode, Align, PolicyType,
    StackTransitionType,
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;

pub struct SidebarWidgets {
    pub container: GtkBox,
    pub thumb_list: ListBox,
    pub outline_list: ListBox,
    pub annot_list: ListBox,
    pub search_list: ListBox,
    pub search_result_label: Label,
}

pub fn build(
    engine: Rc<RefCell<PdfEngine>>,
    drawing_area: &gtk4::DrawingArea,
) -> SidebarWidgets {
    let container = GtkBox::new(Orientation::Vertical, 0);
    container.set_width_request(250);
    container.set_hexpand(false);

    // --- 1. Tab Header ---
    let tab_box = GtkBox::new(Orientation::Horizontal, 0);
    tab_box.add_css_class("linked");
    tab_box.set_halign(Align::Center);
    tab_box.set_margin_top(5);
    tab_box.set_margin_bottom(5);

    let btn_thumbs = create_tab_button("📄", "thumbs");
    let btn_outline = create_tab_button("📑", "outline");
    let btn_annots = create_tab_button("📝", "annots");
    let btn_search = create_tab_button("🔍", "search");

    tab_box.append(&btn_thumbs);
    tab_box.append(&btn_outline);
    tab_box.append(&btn_annots);
    tab_box.append(&btn_search);
    container.append(&tab_box);

    // --- 2. Main Stack ---
    let stack = Stack::new();
    stack.set_vexpand(true);
    stack.set_transition_type(StackTransitionType::SlideLeftRight);

    // Tab 1: Thumbnails
    let thumb_list = ListBox::new();
    thumb_list.set_selection_mode(SelectionMode::Single);
    let thumb_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .child(&thumb_list)
        .build();
    stack.add_named(&thumb_scroll, Some("thumbs"));

    // Tab 2: Outline
    let outline_list = ListBox::new();
    let outline_scroll = ScrolledWindow::builder().child(&outline_list).build();
    stack.add_named(&outline_scroll, Some("outline"));

    // Tab 3: Annotations
    let annot_list = ListBox::new();
    annot_list.set_selection_mode(SelectionMode::None);
    let annot_scroll = ScrolledWindow::builder().child(&annot_list).build();
    stack.add_named(&annot_scroll, Some("annots"));

    // Tab 4: Search
    let search_box = GtkBox::new(Orientation::Vertical, 5);
    let search_entry = SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search..."));
    let search_result_label = Label::new(Some(""));
    search_result_label.add_css_class("caption");
    let search_list = ListBox::new();
    let search_scroll = ScrolledWindow::builder().child(&search_list).vexpand(true).build();
    
    search_box.append(&search_entry);
    search_box.append(&search_result_label);
    search_box.append(&search_scroll);
    stack.add_named(&search_box, Some("search"));

    container.append(&stack);

    // --- Tab Logic ---
    let s = stack.clone(); btn_thumbs.connect_clicked(move |_| s.set_visible_child_name("thumbs"));
    let s = stack.clone(); btn_outline.connect_clicked(move |_| s.set_visible_child_name("outline"));
    let s = stack.clone(); btn_annots.connect_clicked(move |_| s.set_visible_child_name("annots"));
    let s = stack.clone(); btn_search.connect_clicked(move |_| s.set_visible_child_name("search"));

    // --- Click Events ---
    
    // Thumbnails Click
    let eng_thumb = engine.clone();
    let area_thumb = drawing_area.clone();
    thumb_list.connect_row_activated(move |_, row| {
        let idx = row.index(); // 0-based
        if eng_thumb.borrow_mut().jump_to_page(idx) {
            area_thumb.queue_draw();
        }
    });

    // Annotations Click
    let eng_annot = engine.clone();
    let area_annot = drawing_area.clone();
    annot_list.connect_row_activated(move |_, row| {
        // widget_nameに "page_idx,y_pos" を埋め込んでおく戦略
        let name = row.widget_name();
        let s = name.as_str();
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() >= 2 {
            if let Ok(p) = parts[0].parse::<i32>() {
                // ※アノテーションのページ番号はデータ上1-basedだが、内部処理は0-basedに統一注意
                // ここでは保存時に調整済みの前提で index を渡す
                if eng_annot.borrow_mut().jump_to_page(p) {
                    area_annot.queue_draw();
                }
            }
        }
    });

    SidebarWidgets {
        container,
        thumb_list,
        outline_list,
        annot_list,
        search_list,
        search_result_label,
    }
}

fn create_tab_button(label: &str, _name: &str) -> Button {
    Button::builder().label(label).has_frame(false).build()
}

// === 更新ロジックの実装 ===

impl SidebarWidgets {
    pub fn update_thumbnails(&self, total_pages: i32) {
        // クリア
        while let Some(child) = self.thumb_list.first_child() {
            self.thumb_list.remove(&child);
        }
        // 再生成
        for i in 0..total_pages {
            let row = ListBoxRow::new();
            let vbox = GtkBox::new(Orientation::Vertical, 5);
            vbox.set_margin_top(10);
            vbox.set_margin_bottom(10);
            
            // アイコン
            let icon = Image::from_icon_name("text-x-generic-symbolic");
            icon.set_pixel_size(32);
            
            // ラベル
            let label = Label::new(Some(&format!("Page {}", i + 1)));
            label.add_css_class("caption");

            vbox.append(&icon);
            vbox.append(&label);
            row.set_child(Some(&vbox));
            self.thumb_list.append(&row);
        }
    }

    pub fn update_annotations(&self, engine: &PdfEngine) {
        while let Some(child) = self.annot_list.first_child() {
            self.annot_list.remove(&child);
        }

        if engine.annotations.is_empty() {
            let l = Label::new(Some("No annotations"));
            l.set_margin_top(10);
            self.annot_list.append(&l);
            return;
        }

        for ann in &engine.annotations {
            let row = ListBoxRow::new();
            // クリック時のためにデータを埋め込む (pageは保存時1-basedなら -1 して埋め込む)
            row.set_widget_name(&format!("{},{}", ann.page as i32 - 1, ann.y));

            let vbox = GtkBox::new(Orientation::Vertical, 2);

            let page_lbl = Label::new(Some(&format!("Page {}", ann.page)));
            page_lbl.set_halign(Align::Start);
            page_lbl.add_css_class("caption-heading");

            let content_lbl = Label::new(Some(&ann.content));
            content_lbl.set_halign(Align::Start);
            content_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            content_lbl.set_max_width_chars(20);

            vbox.append(&page_lbl);
            vbox.append(&content_lbl);
            row.set_child(Some(&vbox));
            self.annot_list.append(&row);
        }
    }
}