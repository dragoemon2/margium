use gtk4::prelude::*;
use gtk4::{
    gdk, Box as GtkBox, Button, DrawingArea, Entry, GestureClick, Label, 
    Orientation, Popover, TextView, TextBuffer, ScrolledWindow, EventControllerKey
};
use gtk4::ApplicationWindow;
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;
use crate::ui::UiState;

pub fn setup(
    window: &ApplicationWindow,
    drawing_area: &DrawingArea,
    engine: Rc<RefCell<PdfEngine>>,
    ui_state: Rc<RefCell<UiState>>,
) {
    // 1. Popover UIの作成
    let popover = Popover::builder().has_arrow(false).build();
    let menu_box = GtkBox::new(Orientation::Vertical, 0);
    
    // ボタン（状態によって "Add" か "Edit" に切り替わります）
    let action_btn = Button::with_label(" ➕ Add Annotation ");
    action_btn.set_has_frame(false);
    menu_box.append(&action_btn);
    
    popover.set_child(Some(&menu_box));
    popover.set_parent(drawing_area);

    // ★追加: どの状態（新規追加か、既存の編集か）を保持する変数
    let target_annot_id: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // 2. 右クリックのハンドラー
    let right_click = GestureClick::new();
    right_click.set_button(3); // Right click
    
    let ui_click = ui_state.clone();
    let popover_click = popover.clone();
    let eng_click = engine.clone();
    let target_id_click = target_annot_id.clone();
    let btn_click = action_btn.clone();
    let area_click = drawing_area.clone();

    right_click.connect_pressed(move |_, _, x, y| {
        // クリック位置を保存
        ui_click.borrow_mut().last_click_pos = Some((x, y));

        let eng = eng_click.borrow();
        let ui = ui_click.borrow();
        
        // --- 座標変換 (UI座標 → PDF座標) ---
        let scale = ui.scale;
        let area_w = area_click.width() as f64;
        let (pdf_w, _) = eng.get_page_size().unwrap_or((0.0, 0.0));
        let draw_w = pdf_w * scale;
        
        let offset_x = if area_w > draw_w { (area_w - draw_w) / 2.0 } else { 0.0 };
        let offset_y = 20.0;
        
        let pdf_x = (x - offset_x) / scale;
        let pdf_y = (y - offset_y) / scale;

        // --- 当たり判定 ---
        if let Some(hit_id) = eng.hit_test_annotation(pdf_x, pdf_y) {
            // アノテーションの上で右クリックされた場合 -> 「編集」モード
            *target_id_click.borrow_mut() = Some(hit_id);
            btn_click.set_label(" 📝 Edit Annotation ");
        } else {
            // 何もない場所の場合 -> 「追加」モード
            *target_id_click.borrow_mut() = None;
            btn_click.set_label(" ➕ Add Annotation ");
        }

        // Popoverを表示
        let rect = gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
        popover_click.set_pointing_to(Some(&rect));
        popover_click.popup();
    });
    drawing_area.add_controller(right_click);


    // 3. ボタンが押された時（ダイアログを開く）のロジック
    let engine_add = engine.clone();
    let ui_add = ui_state.clone();
    let area_add = drawing_area.clone();
    let popover_action = popover.clone();
    let window_weak = window.downgrade();
    let target_id_action = target_annot_id.clone();

    action_btn.connect_clicked(move |_| {
        popover_action.popdown();

        let ui = ui_add.borrow();
        let (click_x, click_y) = match ui.last_click_pos {
            Some(pos) => pos,
            None => return,
        };
        
        // 座標変換
        let eng = engine_add.borrow();
        let scale = ui.scale;
        let area_w = area_add.width() as f64;
        let (pdf_w, _) = eng.get_page_size().unwrap_or((0.0, 0.0));
        let draw_w = pdf_w * scale;
        let offset_x = if area_w > draw_w { (area_w - draw_w) / 2.0 } else { 0.0 };
        let offset_y = 20.0;
        
        let pdf_x = (click_x - offset_x) / scale;
        let pdf_y = (click_y - offset_y) / scale;

        // 編集モードなら既存のテキストを取得
        let target_id = target_id_action.borrow().clone();
        let initial_text = if let Some(ref id) = target_id {
            eng.annotations.iter().find(|a| &a.id == id).map(|a| a.content.clone()).unwrap_or_default()
        } else {
            String::new()
        };

        // Engineの借用を解除してからダイアログを表示
        drop(eng);

        let parent = window_weak.upgrade().unwrap();
        show_annotation_dialog(
            &parent, 
            engine_add.clone(), 
            area_add.clone(), 
            pdf_x, 
            pdf_y, 
            target_id,     // IDを渡す (Noneなら新規作成)
            &initial_text  // 初期テキスト
        );
    });
}

fn show_annotation_dialog(
    parent: &ApplicationWindow,
    engine: Rc<RefCell<PdfEngine>>,
    drawing_area: DrawingArea,
    x: f64,
    y: f64,
    target_id: Option<String>,
    initial_text: &str,
) {
    let title = if target_id.is_some() { "Edit Annotation" } else { "Add Annotation" };
    
    let dialog = ApplicationWindow::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(350)
        .default_height(200) // 少し高さを広げる
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 10);
    vbox.set_margin_top(20);
    vbox.set_margin_bottom(20);
    vbox.set_margin_start(20);
    vbox.set_margin_end(20);

    // ★変更: Entry の代わりに TextView と TextBuffer を使用
    let text_buffer = TextBuffer::new(None::<&gtk4::TextTagTable>);
    text_buffer.set_text(initial_text);

    let text_view = TextView::with_buffer(&text_buffer);
    text_view.set_wrap_mode(gtk4::WrapMode::WordChar);
    
    // 複数行入力できるようにスクロールウィンドウで囲む
    let scroll = ScrolledWindow::builder()
        .child(&text_view)
        .min_content_height(100)
        .vexpand(true)
        .build();

    let btn_box = GtkBox::new(Orientation::Horizontal, 10);
    btn_box.set_halign(gtk4::Align::Center);
    
    let btn_cancel = Button::with_label("Cancel");
    let btn_ok = Button::with_label("OK");
    dialog.set_default_widget(Some(&btn_ok));

    btn_box.append(&btn_cancel);
    btn_box.append(&btn_ok);
    vbox.append(&Label::new(Some("Enter text (Ctrl+Enter for newline, $...$ for MathJax):")));
    vbox.append(&scroll);
    vbox.append(&btn_box);
    dialog.set_child(Some(&vbox));

    // --- アクションの設定 ---
    let dialog_close = dialog.clone();
    btn_cancel.connect_clicked(move |_| dialog_close.close());

    let dialog_ok = dialog.clone();
    let buffer_clone = text_buffer.clone();
    
    btn_ok.connect_clicked(move |_| {
        let bounds = buffer_clone.bounds();
        let text = buffer_clone.text(&bounds.0, &bounds.1, false).trim().to_string();
        
        if !text.is_empty() {
            let mut eng = engine.borrow_mut();
            if let Some(ref id) = target_id {
                eng.active_annotation_id = Some(id.clone());
                eng.update_active_annotation_content(&text);
            } else {
                if let Err(e) = eng.add_annotation(&text, x, y) {
                    eprintln!("Error: {}", e);
                }
            }
            drawing_area.queue_draw();
        }
        dialog_ok.close();
    });

    // ★追加: キーイベントのカスタマイズ
    let key_ctrl = EventControllerKey::new();
    let btn_ok_clone = btn_ok.clone();
    let tv_clone = text_view.clone();

    key_ctrl.connect_key_pressed(move |_, keyval, _, state| {
        if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
            if state.contains(gdk::ModifierType::CONTROL_MASK) || state.contains(gdk::ModifierType::SHIFT_MASK) {
                btn_ok_clone.emit_clicked();
                return gtk4::glib::Propagation::Stop;
                
            } else {
                tv_clone.buffer().insert_at_cursor("\n");
                return gtk4::glib::Propagation::Stop;
            }
        }
        gtk4::glib::Propagation::Proceed
    });
    text_view.add_controller(key_ctrl);

    dialog.present();
    text_view.grab_focus();
}