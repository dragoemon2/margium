use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, 
    Align, SelectionMode
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;

// 目次データの構造体
#[derive(Clone, Debug)]
pub struct OutlineItem {
    pub title: String,
    pub page_index: Option<i32>, // ページ番号 (ない場合もある)
    pub level: i32,              // 階層レベル (0, 1, 2...)
}

pub struct OutlineWidget {
    pub list: ListBox,
    pub scroll: ScrolledWindow,
    // データを保持しておく (クリック時のページジャンプ用)
    items: RefCell<Vec<OutlineItem>>, 
}

impl OutlineWidget {
    pub fn new(
        engine: Rc<RefCell<PdfEngine>>,
        drawing_area: &gtk4::DrawingArea,
    ) -> Self {
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::Single);
        
        let scroll = ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .build();

        // --- Click Logic ---
        let eng_outline = engine.clone();
        let area_outline = drawing_area.clone();
        
        list.connect_row_activated(move |_, row| {
            let name = row.widget_name();
            if let Ok(page_idx) = name.as_str().parse::<i32>() {
                if let Ok(mut eng) = eng_outline.try_borrow_mut() {
                    if eng.jump_to_page(page_idx) {
                        area_outline.queue_draw();
                    }
                }
            }
        });

        Self { 
            list, 
            scroll,
            items: RefCell::new(Vec::new()),
        }
    }

    pub fn set_outline(&self, items: Vec<OutlineItem>) {
        // 1. データを保存
        *self.items.borrow_mut() = items.clone();

        // 2. リストをクリア
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        if items.is_empty() {
            let l = Label::new(Some("No Outline"));
            l.set_margin_top(10);
            self.list.append(&l);
            return;
        }

        // 3. リストを構築
        for item in items {
            let row = ListBoxRow::new();
            
            // ページ番号があれば埋め込む、なければ空文字
            // (章タイトルだけでリンクがない場合もあるため)
            if let Some(page) = item.page_index {
                row.set_widget_name(&page.to_string());
            }

            let hbox = GtkBox::new(Orientation::Horizontal, 5);
            hbox.set_margin_top(5);
            hbox.set_margin_bottom(5);
            
            // 階層レベルに応じたインデント
            hbox.set_margin_start(item.level * 20); 

            let label = Label::new(Some(&item.title));
            label.set_halign(Align::Start);
            label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            
            hbox.append(&label);
            row.set_child(Some(&hbox));
            
            self.list.append(&row);
        }
    }
}