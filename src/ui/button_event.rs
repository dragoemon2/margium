use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, DrawingArea, TextBuffer, Label, 
    FileChooserDialog, FileChooserAction, ResponseType,
    EventControllerKey, gdk,
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;
use crate::ui::{UiState};
use crate::ui::toolbar::ToolbarWidgets;
use crate::ui::sidebar::SidebarWidgets;

pub fn setup(
    window: &ApplicationWindow,
    engine: Rc<RefCell<PdfEngine>>,
    ui_state: Rc<RefCell<UiState>>,
    widgets: &ToolbarWidgets,
    drawing_area: &DrawingArea,
    sidebar: &Rc<SidebarWidgets>,
    text_buffer: &TextBuffer,
    filename_label: &Label,
) {
    // ---------------------------------------------------------
    // 共通の画面更新関数 (クロージャ)
    // ---------------------------------------------------------
    let update_view = {
        let engine = engine.clone();
        let area = drawing_area.clone();
        let buf = text_buffer.clone();
        let lbl_page = widgets.label_page.clone();
        let lbl_file = filename_label.clone();
        let sb_view = sidebar.clone();

        move || {
            let eng = engine.borrow();
            
            // 1. ラベル更新
            lbl_file.set_text(&eng.status_text());
            lbl_page.set_text(&eng.page_info()); // "1 / 10"

            // 2. 再描画
            area.queue_draw();

            // 3. テキスト更新
            if let Some(text) = eng.get_current_text() {
                buf.set_text(&text);
            } else {
                buf.set_text("");
            }

            // 4. サムネイル更新
            let current_page = engine.borrow().get_current_page_number();
            sb_view.scroll_to_thumbnail(current_page);
            sb_view.update_thumbnails(&eng);
            
        }
    };

    // ---------------------------------------------------------
    // ボタンイベント
    // ---------------------------------------------------------

    // --- Prev Button ---
    let eng_prev = engine.clone();
    let up_prev = update_view.clone();
    widgets.btn_prev.connect_clicked(move |_| {
        if eng_prev.borrow_mut().prev_page() {
            up_prev();
        }
    });

    // --- Next Button ---
    let eng_next = engine.clone();
    let up_next = update_view.clone();
    widgets.btn_next.connect_clicked(move |_| {
        if eng_next.borrow_mut().next_page() {
            up_next();
        }
    });

    // --- Zoom In ---
    let ui_in = ui_state.clone();
    let area_in = drawing_area.clone();
    widgets.btn_zoom_in.connect_clicked(move |_| {
        ui_in.borrow_mut().scale += 0.2;
        area_in.queue_draw();
    });

    // --- Zoom Out ---
    let ui_out = ui_state.clone();
    let area_out = drawing_area.clone();
    widgets.btn_zoom_out.connect_clicked(move |_| {
        let mut ui = ui_out.borrow_mut();
        ui.scale = (ui.scale - 0.2).max(0.4);
        area_out.queue_draw();
    });

    // --- Open File ---
    let eng_open = engine.clone();
    let up_open = update_view.clone();
    let window_weak = window.downgrade();
    
    // ファイル選択ダイアログの処理を関数化（ショートカットからも呼べるように）
    let sidebar_for_open = sidebar.clone();

    let open_action = move || {
        let window = match window_weak.upgrade() { Some(w) => w, None => return };
        let dialog = FileChooserDialog::new(
            Some("Select PDF"), Some(&window), FileChooserAction::Open,
            &[("Cancel", ResponseType::Cancel), ("Open", ResponseType::Accept)]
        );
        let filter = gtk4::FileFilter::new();
        filter.add_mime_type("application/pdf");
        dialog.add_filter(&filter);

        let eng = eng_open.clone();
        let up = up_open.clone();

        let sb = sidebar_for_open.clone();

        dialog.connect_response(move |d, response| {
            if response == ResponseType::Accept {
                if let Some(file) = d.file() {
                    if let Some(path) = file.path() {
                        if let Err(e) = eng.borrow_mut().load_file(path) {
                            eprintln!("Error: {}", e);
                        } else {

                            // サイドバー更新 (Annotationsのみ。Thumbnailsは up() に含まれる)
                            let eng_ref = eng.borrow();
                            sb.update_annotations(&eng_ref);
                            sb.init_thumbnails(eng_ref.get_total_pages());

                            // 画面更新
                            up(); 
                        }
                    }
                }
            }
            d.close();
        });
        dialog.show();
    };

    // ボタンに接続
    let open_action_clone = open_action.clone(); // クローンしてボタン用に使う
    widgets.btn_open.connect_clicked(move |_| {
        open_action_clone();
    });

    // ---------------------------------------------------------
    // ショートカットキー (Window全体のイベント)
    // ---------------------------------------------------------
    let key_controller = EventControllerKey::new();
    
    let eng_key = engine.clone();
    let ui_key = ui_state.clone();
    let area_key = drawing_area.clone();
    let up_key = update_view.clone();
    
    // open_action は Clone ではないので、再度定義するか、Rcで包むなどの工夫が必要ですが、
    // ここではシンプルにもう一度 Dialog ロジックを書くか、Openボタンのクリックを発火させます。
    let btn_open_ref = widgets.btn_open.clone();

    key_controller.connect_key_pressed(move |_, keyval, _keycode, state| {
        let mut eng = eng_key.borrow_mut();
        let handled = match keyval {
            // ページ戻る (Left, K, Up)
            gdk::Key::Left | gdk::Key::k | gdk::Key::Up => {
                if eng.prev_page() {
                    drop(eng); // 借用解放
                    up_key();
                }
                true
            }
            // ページ進む (Right, J, Down)
            gdk::Key::Right | gdk::Key::j | gdk::Key::Down => {
                if eng.next_page() {
                    drop(eng);
                    up_key();
                }
                true
            }
            // ズームイン (+, =)
            gdk::Key::plus | gdk::Key::equal => {
                ui_key.borrow_mut().scale += 0.2;
                area_key.queue_draw();
                true
            }
            // ズームアウト (-)
            gdk::Key::minus => {
                let mut ui = ui_key.borrow_mut();
                ui.scale = (ui.scale - 0.2).max(0.4);
                area_key.queue_draw();
                true
            }
            // ファイルを開く (Ctrl + O)
            gdk::Key::o if state.contains(gdk::ModifierType::CONTROL_MASK) => {
                // ボタンのクリックイベントを発火させる（ロジックを再利用）
                btn_open_ref.emit_clicked();
                true
            }
            _ => false,
        };

        if handled {
            gtk4::glib::Propagation::Stop
        } else {
            gtk4::glib::Propagation::Proceed
        }
    });

    window.add_controller(key_controller);
}