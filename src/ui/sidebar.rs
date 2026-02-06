use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Label, Orientation, Stack, ScrolledWindow, 
    ListBox, ListBoxRow, SearchEntry, Image, SelectionMode, Align, PolicyType,
    StackTransitionType,
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;
use std::cmp::{min, max};
use gtk4::gdk;

pub struct SidebarWidgets {
    pub container: GtkBox,
    pub thumb_list: ListBox,
    pub outline_list: ListBox,
    pub annot_list: ListBox,
    pub search_list: ListBox,
    pub search_result_label: Label,
    pub thumb_scroll: ScrolledWindow,
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


    // 2. ★重要: ScrolledWindowが現在持っている「本物の」Adjustmentを取得する
    let vadj = thumb_scroll.vadjustment();

    // 3. イベント接続 (ロジックは前回と同じ)
    {
        // クローン等の準備
        let list_weak = thumb_list.downgrade();
        let scroll_weak = thumb_scroll.downgrade();
        let engine_clone = engine.clone();
        let debounce_timer = Rc::new(RefCell::new(None::<glib::SourceId>));

        // 取得した vadj に対してシグナルを接続
        vadj.connect_value_changed(move |_| {
            // println!("Scroll detected!"); // これで表示されるはずです

            let timer_store = debounce_timer.clone();
            let eng = engine_clone.clone();
            let list_w = list_weak.clone();
            let scroll_w = scroll_weak.clone();

            if let Some(source_id) = timer_store.borrow_mut().take() {
                source_id.remove();
            }

            let timer_store_for_inner = timer_store.clone();

            let new_source_id = glib::timeout_add_local(
                std::time::Duration::from_millis(200), 
                move || {
                    if let (Some(list), Some(scroll)) = (list_w.upgrade(), scroll_w.upgrade()) {
                        perform_thumbnail_update(&list, &scroll, &eng.borrow());
                    }
                    *timer_store_for_inner.borrow_mut() = None;
                    glib::ControlFlow::Break
                }
            );
            
            *timer_store.borrow_mut() = Some(new_source_id);
        });
    }

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
        thumb_scroll,
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

fn perform_thumbnail_update(
    thumb_list: &ListBox, 
    thumb_scroll: &ScrolledWindow, 
    engine: &PdfEngine
) {
    let current = engine.get_current_page_number();
    let total = engine.get_total_pages();
    
    // 1. スクロール情報を取得
    let vadj = thumb_scroll.vadjustment();
    let scroll_y = vadj.value();       // 現在のスクロール位置 (px)
    let view_height = vadj.page_size(); // 画面の高さ (px)

    // 2. 1行あたりの高さを固定値で定義 (画像100 + 余白など)
    // ※CSSや設定で大きく変えていない限り、固定値で計算するのが一番速くて確実です
    let item_height = 140.0; 

    // 3. 表示範囲の計算 (ここが変更の核心)
    let (min_visible, max_visible) = if view_height < 1.0 {
        // A. 起動直後 (まだ画面高さが0の時)
        // とりあえず現在のページ前後を表示しておく
        let radius = 4;
        (
            max(0, current - radius),
            min(total - 1, current + radius)
        )
    } else {
        // B. 通常時 (スクロール位置から逆算)
        let start_index = (scroll_y / item_height).floor() as i32;
        let count = (view_height / item_height).ceil() as i32;
        
        let buffer = 2; // 上下に少し余裕を持たせる

        (
            max(0, start_index - buffer),
            min(total - 1, start_index + count + buffer)
        )
    };

    // リストの子要素（行）を順番に走査
    let mut i = 0;
    let mut child = thumb_list.first_child();
    
    while let Some(row_widget) = child {
        // ListBoxRow -> Box -> Image を取り出す処理
        if let Some(row) = row_widget.downcast_ref::<ListBoxRow>() {
            if let Some(box_widget) = row.child() {
                if let Some(vbox) = box_widget.downcast_ref::<GtkBox>() {
                    // vboxの最初の子がImageだと仮定
                    if let Some(first_child) = vbox.first_child() {
                        if let Some(image) = first_child.downcast_ref::<Image>() {
                            
                            // ★判定ロジック: 範囲内なら描画、範囲外ならメモリ解放
                            if i >= min_visible && i <= max_visible {
                                // まだ画像がセットされていない（またはプレースホルダー）場合のみ生成
                                // (Paintableがすでにセットされているか確認しても良いが、
                                //  ここでは単純に範囲内ならTexture取得を試みる)
                                //  ※ Texture生成はキャッシュが無いと毎フレーム重いので、
                                //     本来はEngine側でLRUキャッシュを持つのがベストですが、
                                //     ここでは「範囲外を即捨てる」ことでメモリを節約します。
                                
                                // 現在のPaintableが空、またはロード中でなければ再生成しない工夫も可
                                
                                // 画質を落とすために幅を100pxに指定
                                if let Some(texture) = engine.get_page_thumbnail(i, 100.0) {
                                    image.set_paintable(Some(&texture));
                                }
                            } else {
                                // ★範囲外は画像をアンロードしてメモリを軽くする
                                // アイコンに戻す、または None にする
                                image.set_icon_name(Some("text-x-generic-symbolic"));
                            }
                        }
                    }
                }
            }
        }
        
        child = row_widget.next_sibling();
        i += 1;
    }
}

impl SidebarWidgets {
    pub fn init_thumbnails(&self, total_pages: i32) {
        // 既存の中身をクリア
        while let Some(child) = self.thumb_list.first_child() {
            self.thumb_list.remove(&child);
        }

        // 全ページ分の「空の」枠を作る（画像はセットしない）
        for i in 0..total_pages {
            let row = ListBoxRow::new();
            let vbox = GtkBox::new(Orientation::Vertical, 5);
            vbox.set_margin_top(10);
            vbox.set_margin_bottom(10);
            vbox.set_halign(Align::Center);
            
            // 画像ウィジェット（最初はプレースホルダーアイコン）
            let image_widget = Image::new();
            image_widget.set_pixel_size(100); // ★サイズを小さくする（150 -> 100）
            image_widget.set_icon_name(Some("image-loading-symbolic")); // 読み込み中アイコン
            image_widget.add_css_class("thumbnail-img"); // 後でCSSで操作できるようにクラス付与
            
            // ラベル
            let label = Label::new(Some(&format!("{}", i + 1))); // "Page"という文字を削ってスッキリさせる
            label.add_css_class("caption");

            vbox.append(&image_widget);
            vbox.append(&label);
            row.set_child(Some(&vbox));
            
            self.thumb_list.append(&row);
        }
    }

    /// ページ遷移時に呼ぶ。見えている範囲だけ画像を生成し、他は捨てる。
    pub fn update_thumbnails(&self, engine: &PdfEngine) {
        perform_thumbnail_update(&self.thumb_list, &self.thumb_scroll, engine);
        // let current = engine.get_current_page_number();
        // let total = engine.get_total_pages();
        
        // // 1. スクロール情報を取得
        // let vadj = self.thumb_scroll.vadjustment();
        // let scroll_y = vadj.value();       // 現在のスクロール位置 (px)
        // let view_height = vadj.page_size(); // 画面の高さ (px)

        // // 2. 1行あたりの高さを固定値で定義 (画像100 + 余白など)
        // // ※CSSや設定で大きく変えていない限り、固定値で計算するのが一番速くて確実です
        // let item_height = 140.0; 

        // // 3. 表示範囲の計算 (ここが変更の核心)
        // let (min_visible, max_visible) = if view_height < 1.0 {
        //     // A. 起動直後 (まだ画面高さが0の時)
        //     // とりあえず現在のページ前後を表示しておく
        //     let radius = 4;
        //     (
        //         max(0, current - radius),
        //         min(total - 1, current + radius)
        //     )
        // } else {
        //     // B. 通常時 (スクロール位置から逆算)
        //     let start_index = (scroll_y / item_height).floor() as i32;
        //     let count = (view_height / item_height).ceil() as i32;
            
        //     let buffer = 2; // 上下に少し余裕を持たせる

        //     (
        //         max(0, start_index - buffer),
        //         min(total - 1, start_index + count + buffer)
        //     )
        // };

        // // リストの子要素（行）を順番に走査
        // let mut i = 0;
        // let mut child = self.thumb_list.first_child();
        
        // while let Some(row_widget) = child {
        //     // ListBoxRow -> Box -> Image を取り出す処理
        //     if let Some(row) = row_widget.downcast_ref::<ListBoxRow>() {
        //         if let Some(box_widget) = row.child() {
        //             if let Some(vbox) = box_widget.downcast_ref::<GtkBox>() {
        //                 // vboxの最初の子がImageだと仮定
        //                 if let Some(first_child) = vbox.first_child() {
        //                     if let Some(image) = first_child.downcast_ref::<Image>() {
                                
        //                         // ★判定ロジック: 範囲内なら描画、範囲外ならメモリ解放
        //                         if i >= min_visible && i <= max_visible {
        //                             // まだ画像がセットされていない（またはプレースホルダー）場合のみ生成
        //                             // (Paintableがすでにセットされているか確認しても良いが、
        //                             //  ここでは単純に範囲内ならTexture取得を試みる)
        //                             //  ※ Texture生成はキャッシュが無いと毎フレーム重いので、
        //                             //     本来はEngine側でLRUキャッシュを持つのがベストですが、
        //                             //     ここでは「範囲外を即捨てる」ことでメモリを節約します。
                                    
        //                             // 現在のPaintableが空、またはロード中でなければ再生成しない工夫も可
                                    
        //                             // 画質を落とすために幅を100pxに指定
        //                             if let Some(texture) = engine.get_page_thumbnail(i, 100.0) {
        //                                 image.set_paintable(Some(&texture));
        //                             }
        //                         } else {
        //                             // ★範囲外は画像をアンロードしてメモリを軽くする
        //                             // アイコンに戻す、または None にする
        //                             image.set_icon_name(Some("text-x-generic-symbolic"));
        //                         }
        //                     }
        //                 }
        //             }
        //         }
        //     }
            
        //     child = row_widget.next_sibling();
        //     i += 1;
        // }
        
  
    }

    pub fn scroll_to_thumbnail(&self, page_num: i32) {
        // 1. 指定されたページの行（Row）を取得
        if let Some(row) = self.thumb_list.row_at_index(page_num) {
            
            // --- 選択状態にする (ハイライト) ---
            self.thumb_list.select_row(Some(&row));

            // 行の座標をリストボックス基準で取得
            // (rowの左上(0,0)が、list全体の中でどこにあるか)
            if let Some((_, y)) = row.translate_coordinates(&self.thumb_list, 0.0, 0.0) {
                if let Some(row) = self.thumb_list.row_at_index(page_num) {
                    self.thumb_list.select_row(Some(&row));
                    row.grab_focus(); 
                }
            }
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