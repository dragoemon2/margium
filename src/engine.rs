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
use rsvg::SvgHandle;
use pango::FontDescription;
use pangocairo::functions::{create_layout, show_layout};

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

enum DrawPart {
    Text(String, f64), // テキスト内容, 幅
    Math(SvgHandle, f64, f64, f64), // Handle, 描画幅, スケール, 元の高さ
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
            let physical_info = format!("{} / {}", self.current_page + 1, self.total_pages);
            
            if let Some(label) = self.get_page_label(self.current_page) {
                // ラベルがある場合: "Label (Physical / Total)"
                format!("{} ({})", label, physical_info)
            } else {
                // ラベルがない場合: "Physical / Total"
                physical_info
            }
        } else {
            " - ".to_string()
        }
    }

    pub fn get_page_label(&self, page_index: i32) -> Option<String> {
        if let Some(doc) = &self.doc {
            if let Some(page) = doc.page(page_index) {
                // poppler-rs の label() メソッドを使用
                // GString を String に変換
                return page.label().map(|s| s.to_string());
            }
        }
        None
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
        
        for ann in self.annotations.iter().filter(|a| a.page == current_page_u32) {
            context.save().unwrap();
            context.translate(ann.x * scale, ann.y * scale);

            // フォントサイズ (これが全体のスケール基準になります)
            let font_size = ann.font_size.unwrap_or(14.0) as f64;

            // ★方針変更:
            // 中身を \text{ ... } で囲んで、全体を1つのLaTeX数式として処理する。
            // これにより "Hello $x^2$" のような混在も、LaTeXの \text{Hello $x^2$} として正しくレンダリングされる。
            let latex = format!("\\text{{{}}}", ann.content);

            // 1. SVG変換を試みる
            if let Ok(svg_string) = convert_to_svg(&latex) {
                let loader = Loader::new();
                let stream = MemoryInputStream::from_bytes(&Bytes::from(svg_string.as_bytes()));
                
                if let Ok(handle) = loader.read_stream(&stream, None::<&gtk4::gio::File>, None::<&Cancellable>) {
                    let renderer = CairoRenderer::new(&handle);
                    
                    let rect = renderer.intrinsic_dimensions();
                    let w = rect.width.length;
                    let h = rect.height.length;

                    // 2. サイズ計算
                    let target_h = font_size * 1.5; 
                    let s = if h > 0.0 { target_h / h } else { 1.0 };
                    
                    let draw_w = w * s;
                    let draw_h = h * s;

                    // 3. 背景描画 (黄色い付箋)
                    context.set_source_rgba(1.0, 1.0, 0.8, 0.8);
                    context.rectangle(0.0, 0.0, draw_w + 10.0, draw_h); // 少し横に余白(+10.0)
                    context.fill().unwrap();

                    // 4. SVG描画
                    let offset_x = 5.0;
                    let offset_y = (draw_h - (h * s)) / 2.0;

                    context.save().unwrap();
                    context.translate(offset_x, offset_y);
                    context.scale(s, s);
                    
                    let _ = renderer.render_document(
                        context,
                        &cairo::Rectangle::new(0.0, 0.0, w, h)
                    );
                    context.restore().unwrap();
                }
            } else {
                // SVG変換失敗時（LaTeX構文エラーなど）のフォールバック
                // 通常のテキストとして描画
                context.set_source_rgba(1.0, 0.8, 0.8, 0.8); // エラー時は赤っぽい背景
                context.rectangle(0.0, 0.0, 100.0, 20.0);
                context.fill().unwrap();

                context.set_source_rgb(0.0, 0.0, 0.0);
                context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                context.set_font_size(font_size);
                context.move_to(5.0, font_size);
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


}