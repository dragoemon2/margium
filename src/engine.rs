use poppler::{Document, ActionType, LinkMapping};
use std::path::PathBuf;
use cairo::Context;
use crate::annotations::{AnnotationData};

use mathjax_svg::convert_to_svg;
use gtk4::glib::Bytes;
use gtk4::gio::{MemoryInputStream, Cancellable};
use rsvg::{Loader, CairoRenderer};
use gtk4::gdk;
use gtk4::glib;
use poppler::Rectangle;
use std::collections::HashMap;
use crate::ui::sidebar::outline::OutlineItem;

pub struct PdfEngine {
    doc: Option<Document>,
    lo_doc: Option<lopdf::Document>,
    filename: String,
    current_page: i32,
    total_pages: i32,
    filepath: Option<PathBuf>,

    pub annotations: Vec<AnnotationData>,
    pub highlight_rects: Vec<Rectangle>,
    pub search_results_cache: HashMap<i32, Vec<Rectangle>>,
    
}

impl PdfEngine {
    pub fn new() -> Self {
        Self {
            doc: None,
            lo_doc: None,
            filename: String::new(),
            current_page: 0,
            total_pages: 0,
            filepath: None,
            annotations: Vec::new(),
            highlight_rects: Vec::new(),
            search_results_cache: HashMap::new(),
        }
    }

    pub fn jump_to_page(&mut self, page_index: i32) -> bool {
        if page_index >= 0 && page_index < self.total_pages {
            self.current_page = page_index;
            self.update_highlights_for_current_page();
            return true;
        }
        false
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<(), String> {
        let uri = format!("file://{}", path.to_str().unwrap_or(""));
        
        match Document::from_file(&uri, None) {
            Ok(doc) => {
                self.total_pages = doc.n_pages();
                self.filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.current_page = 0;
                self.doc = Some(doc);
                self.filepath = Some(path);
                Ok(())
            }
            Err(e) => Err(format!("PDF Error: {}", e)),
        }
    }

    pub fn set_annotations(&mut self, annots: Vec<AnnotationData>) {
        self.annotations = annots;
    }

    pub fn next_page(&mut self) -> bool {
        if self.current_page < self.total_pages - 1 {
            self.jump_to_page(self.current_page + 1);
            return true;
        }
        false
    }

    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.jump_to_page(self.current_page - 1);
            return true;
        }
        false
    }

    pub fn status_text(&self) -> String {
        if self.total_pages > 0 {
            format!("{}", self.filename)
        } else {
            " ".to_string()
        }
    }

    pub fn page_info(&self) -> String {
        if self.total_pages > 0 {
            format!("{} / {}", self.current_page + 1, self.total_pages)
        } else {
            " - ".to_string()
        }
    }

    pub fn get_page_size(&self) -> Option<(f64, f64)> {
        if let Some(doc) = &self.doc {
            if let Some(page) = doc.page(self.current_page) {
                return Some(page.size());
            }
        }
        None
    }

    pub fn get_total_pages(&self) -> i32 {
        self.total_pages
    }

    pub fn get_text_of_page(&self, page_index: i32) -> Option<String> {
        if let Some(doc) = &self.doc {
            if let Some(page) = doc.page(page_index) {
                return page.text().map(|s| s.to_string());
            }
        }
        None
    }

    pub fn get_current_text(&self) -> Option<String> {
        self.get_text_of_page(self.current_page)
    }

    pub fn get_current_page_number(&self) -> i32 {
        self.current_page
    }


    pub fn add_annotation(&mut self, text: &str, x: f64, y: f64) -> Result<(), String> {
        // 1. リストに追加
        self.annotations.push(AnnotationData {
            page: (self.current_page + 1) as u32, // lopdfは1-based
            x,
            y,
            content: text.to_string(),
            font_size: Some(14.0),
            id: uuid::Uuid::nil().to_string(),
            object_id: None,
        });

        Ok(())
    }

    // 検索結果を丸ごと受け取るメソッド
    pub fn set_all_search_results(&mut self, results: HashMap<i32, Vec<Rectangle>>) {
        self.search_results_cache = results;
        // 現在のページに結果があれば即反映
        self.update_highlights_for_current_page();
    }

    // 検索クリア
    pub fn clear_search_results(&mut self) {
        self.search_results_cache.clear();
        self.highlight_rects.clear();
    }
    
    // ヘルパーメソッド: 現在のページに対応するハイライトをセット
    fn update_highlights_for_current_page(&mut self) {
        if let Some(rects) = self.search_results_cache.get(&self.current_page) {
            self.highlight_rects = rects.clone();
        } else {
            self.highlight_rects.clear();
        }
    }

    pub fn draw(&self, context: &Context, area_width: f64, area_height: f64, scale: f64) {
        // ---------------------- PDF描画処理 ----------------------

        // 1. 背景をダークグレーで塗りつぶす
        context.set_source_rgb(0.2, 0.2, 0.2);
        context.paint().expect("Painting failed");

        if let Some(doc) = &self.doc {
            if let Some(page) = doc.page(self.current_page) {
                let (pdf_w, pdf_h) = page.size();
                
                // 2. 描画後のサイズを計算
                let draw_w = pdf_w * scale;
                let draw_h = pdf_h * scale;

                // 3. 中央寄せのためのオフセット計算 (X軸)
                // 画面幅の方が広い場合のみ中央に寄せる。画面の方が狭いなら左端(0)から。
                let offset_x = if area_width > draw_w {
                    (area_width - draw_w) / 2.0
                } else {
                    0.0
                };
                
                // 上部は少し余白(20px)を空ける
                let offset_y = 20.0;

                // 4. 座標系を変換 (移動 -> 拡大)
                context.save().unwrap(); // 状態保存
                
                context.translate(offset_x, offset_y);

                // PDFの影を描画 (オプション: ちょっと立体的に見える)
                context.set_source_rgba(0.0, 0.0, 0.0, 0.5);
                context.rectangle(5.0, 5.0, draw_w, draw_h);
                context.fill().unwrap();

                // 用紙の白背景を描画
                context.set_source_rgb(1.0, 1.0, 1.0);
                context.rectangle(0.0, 0.0, draw_w, draw_h);
                context.fill().unwrap();

                // 拡大適用
                context.scale(scale, scale);
                
                // PDFの中身を描画
                page.render(context);

                // アノテーションを描画
                self.draw_custom_annotations(context, scale);

                // 検索ハイライトを描画
                if !self.highlight_rects.is_empty() {
                    context.save().unwrap();
                    context.set_source_rgba(1.0, 0.0, 0.0, 0.5); 
                    
                    // ページの本来の高さを取得（これで反転計算する）
                    let (_, page_h) = page.size();

                    for rect in &self.highlight_rects {
                        // --- 座標変換ロジック ---
                        // PDF: 左下が(0,0)。Yは上に向かって増える。
                        // Cairo: 左上が(0,0)。Yは下に向かって増える。
                        
                        // 1. PDF座標系での「上端」と「下端」を整理
                        // (Popplerの矩形は y1 < y2 とは限らないため念のため min/max を使う)
                        let pdf_y_bottom = rect.y1().min(rect.y2());
                        let pdf_y_top = rect.y1().max(rect.y2());
                        
                        // 2. Cairo座標系へ変換
                        // Cairoでの描画開始位置(Y) = ページ高さ - PDFでの上端
                        let cairo_y = page_h - pdf_y_top;
                        
                        // 高さはそのまま差分
                        let height = pdf_y_top - pdf_y_bottom;
                        let width = (rect.x2() - rect.x1()).abs();

                        // 3. 描画
                        context.rectangle(rect.x1(), cairo_y, width, height);
                        context.fill().unwrap();
                    }
                    context.restore().unwrap();
                }
                
                context.restore().unwrap(); // 状態復帰
            }
        } else {
            // PDFがない時のメッセージ（中央寄せ）
            context.set_source_rgb(0.7, 0.7, 0.7);
            context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            context.set_font_size(24.0);
            
            let text = "Drag & Drop or Click 'Open'";
            let extents = context.text_extents(text).unwrap();
            
            let x = (area_width - extents.width()) / 2.0;
            let y = area_height / 2.0;
            
            context.move_to(x, y);
            context.show_text(text).unwrap();
        }

    }

    fn draw_custom_annotations(&self, context: &Context, scale: f64) {
        let current_page_u32 = (self.current_page + 1) as u32;
        
        // 現在のページのアノテーションのみ抽出
        for ann in self.annotations.iter().filter(|a| a.page == current_page_u32) {
            
            context.save().unwrap();
            
            // 座標変換: アノテーションの位置へ移動 (scaleも考慮)
            context.translate(ann.x * scale, ann.y * scale);

            // 数式かどうかの判定 ($$で囲まれているか)
            if ann.content.starts_with("$$") && ann.content.ends_with("$$") {
                let latex = &ann.content[2..ann.content.len()-2];
                
                // MathJaxによるSVG変換
                if let Ok(svg_string) = convert_to_svg(latex) {
                    let loader = Loader::new();
                    let stream = MemoryInputStream::from_bytes(&Bytes::from(svg_string.as_bytes()));
                    
                    if let Ok(handle) = loader.read_stream(
                        &stream, 
                        None::<&gtk4::gio::File>, // ← ここを修正
                        None::<&Cancellable>
                    ) {
                        let renderer = CairoRenderer::new(&handle);
                        
                        let rect = renderer.intrinsic_dimensions();
                        let w = rect.width.length;
                        let h = rect.height.length;
                        
                        // フォントサイズに合わせてスケール調整 (目標高さ: 30px程度)
                        let target_h = 30.0; // 本来は ann.font_size から計算しても良い
                        let s = if h > 0.0 { target_h / h } else { 1.0 };
                        
                        context.scale(s, s);
                        
                        // 背景 (白の透過) - 数式を見やすくする
                        context.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                        context.rectangle(0.0, 0.0, w, h);
                        context.fill().unwrap();

                        // 数式描画
                        let _ = renderer.render_document(
                                context,
                                &cairo::Rectangle::new(0.0, 0.0, w, h) // ← newメソッドを使う
                            );
                    }
                } else {
                    // SVG変換エラー時のフォールバック
                    context.set_source_rgb(1.0, 0.0, 0.0);
                    context.show_text("Math Error").unwrap();
                }
            } else {
                // 通常のテキスト描画 (Pangoを使用するのがベストですが、簡易的にCairoで)
                context.set_source_rgba(1.0, 1.0, 0.8, 0.8); // 付箋っぽい背景
                context.rectangle(0.0, 0.0, 100.0, 20.0);    // 仮のサイズ
                context.fill().unwrap();

                context.set_source_rgb(0.0, 0.0, 0.0);
                context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                context.set_font_size(14.0);
                context.move_to(5.0, 15.0);
                context.show_text(&ann.content).unwrap();
            }
            context.restore().unwrap();
        }
    }


    
    pub fn get_page_thumbnail(&self, page_num: i32, target_width: f64) -> Option<gdk::Texture> {
        let doc = self.doc.as_ref()?;
        let page = doc.page(page_num)?;

        let (w, h) = page.size();
        // ★ 高速化ポイント1: 
        // サムネイルならそこまで高画質でなくて良いので、計算上のスケールを少し小さく見積もる手もありますが、
        // ここでは受け取った target_width に忠実にしつつ、後でUI側で小さい値を渡すようにします。
        let scale = target_width / w;
        let target_height = h * scale;

        let mut surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32, 
            target_width as i32, 
            target_height as i32
        ).ok()?;

        let context = cairo::Context::new(&surface).ok()?;
        
        // ★ 高速化ポイント2: 描画品質を下げる（サムネイルならこれで十分）
        context.set_antialias(cairo::Antialias::None); // アンチエイリアス無効化で高速化
        context.set_source_rgb(1.0, 1.0, 1.0); // 背景を白で塗る（透明だと重くなる場合があるため）
        context.rectangle(0.0, 0.0, target_width, target_height);
        context.fill().unwrap();

        context.scale(scale, scale);
        
        // PDFレンダリング
        page.render(&context);
        
        drop(context);
        
        let stride = surface.stride();
        let width = surface.width();
        let height = surface.height();
        let data = surface.data().ok()?;
        let bytes = glib::Bytes::from(&*data);

        let texture = gdk::MemoryTexture::new(
            width,
            height,
            gdk::MemoryFormat::B8g8r8a8,
            &bytes,
            stride as usize,
        );

        Some(texture.into())
    }

    pub fn get_filepath(&self) -> Option<PathBuf> {
        self.filepath.clone()
    }

    // pub fn get_outline(&self) -> Vec<OutlineItem> {
    //     let mut result = Vec::new();
        
    //     if let Some(doc) = &self.doc {
    //          // poppler-rs のバージョンによってメソッドが異なります
    //          // doc.find_dest() ではなく、イテレータで取得するのが一般的
             
    //          if let Some(iter) = doc.index_iter() {
    //              self.walk_index(iter, 0, &mut result);
    //          }
    //     }
    //     result
    // }

    // // // 再帰的にツリーを辿る
    // // fn walk_index(&self, mut iter: poppler::IndexIter, level: i32, list: &mut Vec<OutlineItem>) {
    // //     // 全ての兄弟ノードをループ
    // //     loop {
    // //         // 現在のノードのアクションを取得
    // //         let mut page_index = None;
            
    // //         if let Some(action) = iter.action() {
    // //             // アクションからページ番号を解決
    // //             // (GotoDest, Named などがある)
    // //             match action.type_() {
    // //                 poppler::ActionType::GotoDest => {
    // //                     if let Some(dest) = action.as_goto_dest() {
    // //                          if let Some(dest) = dest.dest() {
    // //                              // Named Destinationの場合、ドキュメントから解決が必要
    // //                              if let Some(resolved) = self.doc.as_ref().and_then(|d| d.find_dest(dest.as_str())) {
    // //                                  page_index = Some(resolved.page_num() - 1); // 1-based -> 0-based
    // //                              }
    // //                          } else {
    // //                              // 直接ページ指定の場合
    // //                              // dest.page_num() があれば使う (バージョンによる)
    // //                          }
    // //                     }
    // //                 },
    // //                 // Namedアクション
    // //                 poppler::ActionType::Named => {
    // //                     if let Some(named) = action.as_named() {
    // //                         if let Some(name) = named.named_dest() {
    // //                             if let Some(resolved) = self.doc.as_ref().and_then(|d| d.find_dest(&name)) {
    // //                                 page_index = Some(resolved.page_num() - 1);
    // //                             }
    // //                         }
    // //                     }
    // //                 }
    // //                 _ => {}
    // //             }
    // //         }

    // //         // タイトル取得
    // //         // (APIによっては iter.action().title() だったり iter.title() だったりします)
    // //         // ここでは Action からタイトルが取れると仮定、または IndexIter 自体が持っている場合
    // //         // poppler-rs 0.22では `iter.action().map(|a| a.title())` 等
            
    // //         // 安全策: action経由でタイトルが取れればそれを、ダメなら "Untitled"
    // //         let title = if let Some(action) = iter.action() {
    // //             action.title().unwrap_or("Untitled".to_string())
    // //         } else {
    // //             "Untitled".to_string()
    // //         };

    // //         list.push(OutlineItem {
    // //             title,
    // //             page_index,
    // //             level,
    // //         });

    // //         // 子ノードがあれば再帰
    // //         if let Some(child_iter) = iter.child() {
    // //             self.walk_index(child_iter, level + 1, list);
    // //         }

    // //         // 次の兄弟へ (なければ終了)
    // //         if !iter.next() {
    // //             break;
    // //         }
    // //     }
    // // }

}