use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, DrawingArea, FileChooserAction, FileChooserDialog,
    ResponseType, ScrolledWindow, Button, Label, Orientation, 
    EventControllerKey, EventControllerScroll, EventControllerScrollFlags,
    Separator, DropDown, StringList, GestureClick, Popover,
    Entry, Window
};
use std::cell::RefCell;
use std::rc::Rc;
use crate::annotations::AnnotationData;
use crate::annotations;

use crate::engine::PdfEngine;

// ズーム倍率や表示設定を管理するUI専用の状態
struct UiState {
    scale: f64,
    last_click_pos: Option<(f64, f64)>,
}

pub fn build(app: &Application) {
    // ロジック初期化
    let engine = Rc::new(RefCell::new(PdfEngine::new()));
    let ui_state = Rc::new(RefCell::new(UiState {
        scale: 1.0,
        last_click_pos: None,
    }));

    // ============================================================
    // 1. レイアウト構築 (Reactの構造を再現)
    // ============================================================

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Margium")
        .default_width(1000)
        .default_height(800)
        .build();

    // 全体のコンテナ (flex-direction: row に相当)
    let main_layout = gtk4::Box::new(Orientation::Horizontal, 0);
    window.set_child(Some(&main_layout));

    // --- A. サイドバー (Left) ---
    let sidebar = gtk4::Box::new(Orientation::Vertical, 0);
    sidebar.set_width_request(250); // 幅を固定
    
    // サイドバーの中身（ダミー）
    let sidebar_label = Label::new(Some("Sidebar"));
    sidebar_label.set_margin_top(10);
    sidebar.append(&sidebar_label);
    
    main_layout.append(&sidebar);

    // 境界線
    let v_sep = Separator::new(Orientation::Vertical);
    main_layout.append(&v_sep);

    // --- B. メインコンテンツ (Right) ---
    let main_content = gtk4::Box::new(Orientation::Vertical, 0);
    main_content.set_hexpand(true); // 残りの幅を埋める
    main_layout.append(&main_content);

    // --- B-1. ツールバー (Top) ---
    let toolbar = gtk4::Box::new(Orientation::Horizontal, 10);
    toolbar.set_margin_top(8);
    toolbar.set_margin_bottom(8);
    toolbar.set_margin_start(10);
    toolbar.set_margin_end(10);

    // ファイル名表示 (Left)
    let filename_label = Label::new(Some("No File Selected"));
    filename_label.set_attributes(Some(&pango::AttrList::new())); // BoldにするならPango属性が必要（省略）
    toolbar.append(&filename_label);

    // スペーサー (左右を離すため、真ん中で伸びる透明な箱)
    let spacer = gtk4::Box::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    toolbar.append(&spacer);

    // ボタン群 (Right)
    let btn_open = Button::with_label("📂 Open");
    let btn_save = Button::with_label("💾 Save"); // 機能未実装のため飾り
    let btn_save_as = Button::with_label("💾 Save As"); // 飾り
    let btn_zoom_in = Button::with_label("🔍 Zoom In");
    let btn_zoom_out = Button::with_label("🔍 Zoom Out");
    
    // 言語選択 (Dropdown)
    let lang_list = StringList::new(&["English", "Japanese"]);
    let lang_dropdown = DropDown::new(Some(lang_list), Option::<gtk4::Expression>::None);

    // React: disabled={!pdfPath} の再現
    btn_save.set_sensitive(false);
    btn_save_as.set_sensitive(false);

    toolbar.append(&btn_open);
    toolbar.append(&btn_save);
    toolbar.append(&btn_save_as);
    toolbar.append(&btn_zoom_in);
    toolbar.append(&btn_zoom_out);
    toolbar.append(&lang_dropdown);

    main_content.append(&toolbar);
    
    // ツールバー下の線
    let h_sep = Separator::new(Orientation::Horizontal);
    main_content.append(&h_sep);

    // --- B-2. PDF表示エリア (Bottom) ---
    let drawing_area = DrawingArea::new();
    drawing_area.set_content_width(800);
    drawing_area.set_content_height(1000);

    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .child(&drawing_area)
        .vexpand(true) // 高さいっぱいまで広げる
        .build();

    main_content.append(&scrolled_window);

    // ポップオーバーメニュー
    let popover = Popover::builder()
        .has_arrow(false)
        .build();
    let menu_box = gtk4::Box::new(Orientation::Vertical, 0);

    // メニュー項目
    let add_annot_btn = Button::with_label(" ➕ Add Annotation ");
    add_annot_btn.set_has_frame(false); // メニュー項目っぽく枠線を消す
    
    menu_box.append(&add_annot_btn);
    popover.set_child(Some(&menu_box));
    
    // DrawingAreaを親にする
    popover.set_parent(&drawing_area);


    // ============================================================
    // 2. イベントハンドリング & ロジック接続
    // ============================================================

    // 描画処理 (Engine + UiState)
    let engine_draw = engine.clone();
    let ui_draw = ui_state.clone();
    drawing_area.set_draw_func(move |area, context, w, h| {
        let eng = engine_draw.borrow();
        let ui = ui_draw.borrow();
        
        // Engineに描画させる
        eng.draw(context, w as f64, h as f64, ui.scale);

        // 必要ならエリアの高さを確保する処理（簡易実装）
        // PDFの高さよりDrawingAreaが小さければ、DrawingAreaを広げる要求を出す
        if let Some((_, pdf_h)) = eng.get_page_size() {
            let required_h = (pdf_h * ui.scale) as i32 + 40; // 上下余白分
            if h < required_h {
                area.set_content_height(required_h);
            }
        }
    });

    // 画面更新ヘルパー
    let update_view = {
        let area = drawing_area.clone();
        let label = filename_label.clone();
        let engine = engine.clone();
        let btn_save = btn_save.clone();
        
        move || {
            let eng = engine.borrow();
            label.set_text(&eng.status_text());
            
            // ボタン有効化
            // (Engineにis_loadedフラグがあればそれを使うが、ここでは簡易判定)
            btn_save.set_sensitive(true);

            area.queue_draw();
        }
    };

    // --- ボタンアクション ---

    // Open
    let engine_open = engine.clone();
    let update_open = update_view.clone();
    let window_weak = window.downgrade();

    let area_open = drawing_area.clone();
    btn_open.connect_clicked(move |_| {
        let window = match window_weak.upgrade() { Some(w) => w, None => return };

        let dialog = FileChooserDialog::new(
            Some("Select PDF"), Some(&window), FileChooserAction::Open,
            &[("Cancel", ResponseType::Cancel), ("Open", ResponseType::Accept)]
        );
        let filter = gtk4::FileFilter::new();
        filter.add_mime_type("application/pdf");
        dialog.add_filter(&filter);

        // クローン祭り（クロージャ内で使うため）
        let eng = engine_open.clone();
        let up = update_open.clone();
        let area = area_open.clone();

        dialog.connect_response(move |d, response| {
            if response == ResponseType::Accept {
                if let Some(file) = d.file() {
                    if let Some(path) = file.path() {
                        // 1. まずPDFを表示する (同期処理・高速)
                        //    Popplerでの描画準備だけ済ませる
                        let path_for_thread = path.to_str().unwrap().to_string(); // スレッドに渡す用
                        
                        if let Err(e) = eng.borrow_mut().load_file(path) {
                            eprintln!("Load error: {}", e);
                            d.close();
                            return;
                        }
                        // ここで一旦描画更新！ ユーザーにはPDFが表示される
                        up(); 


                        // 2. バックグラウンドでアノテーションを読み込む (非同期処理・低速)
                        
                        // メインスレッドとの通信チャンネルを作成
                        let (sender, receiver) = async_channel::unbounded::<Result<Vec<AnnotationData>, String>>();
                        
                        let eng_async = eng.clone();
                        let area_async = area.clone();

                        // メインスレッド側で待機するタスク (UI更新用)
                        // spawn_local はメインループ上で非同期タスクを実行します
                        gtk4::glib::MainContext::default().spawn_local(async move {
                            // 受信ループ
                            while let Ok(result) = receiver.recv().await {
                                match result {
                                    Ok(annots) => {
                                        println!("Background: Loaded {} annotations.", annots.len());
                                        eng_async.borrow_mut().set_annotations(annots);
                                        area_async.queue_draw();
                                    }
                                    Err(e) => {
                                        eprintln!("Background Error: {}", e);
                                    }
                                }
                            }
                        });
                        
                        // 重い処理を実行するワーカースレッド (OSスレッド)
                        std::thread::spawn(move || {
                            println!("Background: Start loading annotations...");
                            
                            let result = annotations::load_annotations(path_for_thread);
                            
                            let _ = sender.send_blocking(result);
                        });
                    }
                }
            }
            d.close();
        });
        dialog.show();
    });

    // Zoom In
    let ui_zoom_in = ui_state.clone();
    let area_zoom_in = drawing_area.clone();
    btn_zoom_in.connect_clicked(move |_| {
        let mut s = ui_zoom_in.borrow_mut();
        s.scale += 0.2;
        area_zoom_in.queue_draw();
    });

    // Zoom Out
    let ui_zoom_out = ui_state.clone();
    let area_zoom_out = drawing_area.clone();
    btn_zoom_out.connect_clicked(move |_| {
        let mut s = ui_zoom_out.borrow_mut();
        s.scale = (s.scale - 0.2).max(0.4); // React: Math.max(0.4, s - 0.2)
        area_zoom_out.queue_draw();
    });

    // --- キーボード & スクロール操作 (前回のロジックを保持) ---
    
    // スクロールでページ送り
    let engine_scroll = engine.clone();
    let update_scroll = update_view.clone();
    let scroll_controller = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    scroll_controller.connect_scroll(move |_, _, dy| {
        let mut eng = engine_scroll.borrow_mut();
        let changed = if dy > 0.0 { eng.next_page() } else { eng.prev_page() };
        
        if changed {
            drop(eng);
            update_scroll();
        }
        gtk4::glib::Propagation::Stop
    });
    window.add_controller(scroll_controller);

    // 矢印キーでページ送り
    let engine_key = engine.clone();
    let update_key = update_view.clone();
    let key_controller = EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        let mut eng = engine_key.borrow_mut();
        let changed = match key.name().as_deref() {
            Some("Right") | Some("j") | Some("Down") => eng.next_page(),
            Some("Left") | Some("k") | Some("Up") => eng.prev_page(),
            _ => return gtk4::glib::Propagation::Proceed,
        };
        if changed {
            drop(eng);
            update_key();
        }
        gtk4::glib::Propagation::Stop
    });
    window.add_controller(key_controller);

    // --- ポップオーバーメニュー ---

    let right_click = GestureClick::new();
    right_click.set_button(3); // 3 = 右クリック
    
    let ui_click = ui_state.clone();
    let popover_click = popover.clone();

    right_click.connect_pressed(move |gesture, _, x, y| {
        // クリック位置を保存
        ui_click.borrow_mut().last_click_pos = Some((x, y));

        // クリック位置にメニューを表示
        let rect = gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
        popover_click.set_pointing_to(Some(&rect));
        popover_click.popup();
    });
    drawing_area.add_controller(right_click);

    // アノテーション追加
    let engine_add = engine.clone();
    let ui_add = ui_state.clone();
    let area_add = drawing_area.clone();
    let popover_action = popover.clone();
    let window_weak = window.downgrade();

    add_annot_btn.connect_clicked(move |_| {
        popover_action.popdown(); // メニューを閉じる

        // 1. まず座標計算を済ませる
        let ui = ui_add.borrow();
        let eng_ref = engine_add.borrow(); // サイズ取得のために一時借用
        
        let (click_x, click_y) = match ui.last_click_pos {
            Some(pos) => pos,
            None => return,
        };

        let (pdf_w, _) = match eng_ref.get_page_size() {
            Some(size) => size,
            None => return,
        };
        drop(eng_ref); // 借用解放

        let area_w = area_add.width() as f64;
        let draw_w = pdf_w * ui.scale;
        
        let offset_x = if area_w > draw_w { (area_w - draw_w) / 2.0 } else { 0.0 };
        let offset_y = 20.0;

        let pdf_x = (click_x - offset_x) / ui.scale;
        let pdf_y = (click_y - offset_y) / ui.scale;

        // 有効範囲外なら何もしない
        if pdf_x < 0.0 || pdf_y < 0.0 {
            return;
        }

        // 2. 入力ダイアログを作成
        let parent_window = window_weak.upgrade();
        let dialog = Window::builder()
            .title("Annotation Text")
            .transient_for(&parent_window.unwrap()) // 親ウィンドウの手前に表示
            .modal(true) // 操作をロック
            .default_width(300)
            .default_height(100)
            .build();

        let vbox = gtk4::Box::new(Orientation::Vertical, 10);
        vbox.set_margin_top(20);
        vbox.set_margin_bottom(20);
        vbox.set_margin_start(20);
        vbox.set_margin_end(20);

        let label = Label::new(Some("Enter text:"));
        let entry = Entry::new();
        entry.set_text("New Note"); // デフォルト値
        entry.set_activates_default(true); // Enterキーで確定できるようにする

        let btn_box = gtk4::Box::new(Orientation::Horizontal, 10);
        btn_box.set_halign(gtk4::Align::Center);
        
        let btn_cancel = Button::with_label("Cancel");
        let btn_ok = Button::with_label("OK");
        // EnterキーでOKボタンが押されるように設定
        dialog.set_default_widget(Some(&btn_ok)); 

        btn_box.append(&btn_cancel);
        btn_box.append(&btn_ok);

        vbox.append(&label);
        vbox.append(&entry);
        vbox.append(&btn_box);
        dialog.set_child(Some(&vbox));

        // 3. アクション定義
        
        // OKボタンの処理
        let entry_clone = entry.clone();
        let dialog_close = dialog.clone();
        let engine_inner = engine_add.clone();
        let area_inner = area_add.clone();

        btn_ok.connect_clicked(move |_| {
            let text = entry_clone.text();
            if !text.is_empty() {
                let mut eng = engine_inner.borrow_mut();
                if let Err(e) = eng.add_annotation(&text, pdf_x, pdf_y) {
                    eprintln!("Error adding annotation: {}", e);
                } else {
                    area_inner.queue_draw();
                }
            }
            dialog_close.close();
        });

        // Cancelボタンの処理
        let dialog_cancel = dialog.clone();
        btn_cancel.connect_clicked(move |_| {
            dialog_cancel.close();
        });

        dialog.present();
    });


    window.present();
}