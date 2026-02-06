use lopdf::{Document, Object, Dictionary, StringFormat};
use std::collections::{HashMap};
use std::str;
use uuid::Uuid;
use std::{time};

// 外部で使うために pub をつける
#[derive(Debug, Clone)]
pub struct AnnotationData {
    pub page: u32,       // 1-based index (lopdf仕様)
    pub x: f64,
    pub y: f64,          // UI座標 (Top-Left 0,0)
    pub content: String,
    pub font_size: Option<f32>,
    pub id: String,      // UIでの識別用ID
    pub object_id: Option<(u32, u16)>, 
}

// ヘルパー関数: Objectからf64を取り出す
fn get_f64(obj: &Object) -> f64 {
    match *obj {
        Object::Real(v) => v as f64,
        Object::Integer(v) => v as f64,
        _ => 0.0,
    }
}

fn parse_font_size_from_da(da: &str) -> Option<f32> {
    let parts: Vec<&str> = da.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "Tf" && i > 0 {
            if let Ok(size) = parts[i - 1].parse::<f32>() {
                return Some(size);
            }
        }
    }
    None
}

fn get_page_height(doc: &Document, page_id: lopdf::ObjectId) -> Option<f32> {
    let page_obj = doc.get_object(page_id).ok()?;
    let page_dict = page_obj.as_dict().ok()?;
    
    let media_box = page_dict.get(b"MediaBox").ok().and_then(|o| o.as_array().ok())?;
    
    if media_box.len() >= 4 {
        let y1 = media_box[1].as_f32().ok()?;
        let y2 = media_box[3].as_f32().ok()?;
        Some((y2 - y1).abs())
    } else {
        None
    }
}

pub fn load_annotations(path: String) -> Result<Vec<AnnotationData>, String> {
    // ignore_xref_streams=true にすると、一部の不正なPDFで高速になる場合がありますが、
    // 基本は load() でOKです。lopdfはデフォルトで遅延ロードを行います。
    let now = time::Instant::now();

    let doc = Document::load(&path).map_err(|e| e.to_string())?;
    let mut annotations = Vec::new();

    println!("Loaded document in {:?}", now.elapsed());

    for (page_num, page_id) in doc.get_pages() {
        let page_dict = doc.get_object(page_id).and_then(|o| o.as_dict()).map_err(|e| e.to_string())?;
        
        let media_box = page_dict.get(b"MediaBox")
            .and_then(|o| o.as_array())
            .map(|a| a.iter().map(|f| get_f64(f)).collect::<Vec<f64>>())
            .unwrap_or(vec![0.0, 0.0, 595.0, 842.0]);
        let page_height = media_box[3];

        if let Ok(annots_obj) = page_dict.get(b"Annots") {
            // Annotsが配列か参照かを解決
            let annots_list = match *annots_obj {
                Object::Reference(id) => {
                    doc.get_object(id).and_then(|o| o.as_array()).ok()
                },
                Object::Array(ref arr) => {
                    Some(arr)
                },
                _ => None
            };

            if let Some(annots_arr) = annots_list {
                for annot_ref in annots_arr {
                    // 参照(Reference)の場合はIDを取得、直接埋め込みの場合はIDなし
                    let (annot_obj_result, obj_id) = match *annot_ref {
                        Object::Reference(id) => (doc.get_object(id), Some(id)),
                        _ => (Ok(annot_ref), None)
                    };

                    if let Ok(annot_obj) = annot_obj_result {
                        if let Ok(annot_dict) = annot_obj.as_dict() {
                            if let (Ok(subtype), Ok(contents), Ok(rect)) = (
                                annot_dict.get(b"Subtype"),
                                annot_dict.get(b"Contents"),
                                annot_dict.get(b"Rect")
                            ) {
                                if subtype.as_name().unwrap_or(&[]) == b"FreeText" {
                                    let content_bytes = contents.as_str().unwrap_or(b"");
                                    let text = String::from_utf8_lossy(content_bytes).to_string();

                                    let mut font_size = None;
                                    if let Ok(da_obj) = annot_dict.get(b"DA") {
                                        let da_str = String::from_utf8_lossy(da_obj.as_str().unwrap_or(b""));
                                        font_size = parse_font_size_from_da(&da_str);
                                    }

                                    if let Ok(rect_arr) = rect.as_array() {
                                        let x_pdf = get_f64(&rect_arr[0]);
                                        let y_pdf_top = get_f64(&rect_arr[3]);
                                        let y_web = page_height - y_pdf_top;

                                        annotations.push(AnnotationData {
                                            page: page_num,
                                            x: x_pdf,      
                                            y: y_web,      
                                            content: text,
                                            font_size: font_size,
                                            id: Uuid::nil().to_string(), // UI用ID（既存のものがあればそれを使う）
                                            object_id: obj_id, // 【重要】PDF内部IDを保存
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println!("Loaded annotations in {:?}", now.elapsed());
    Ok(annotations)
}

pub fn save_pdf_with_annotations(path: String, annotations: Vec<AnnotationData>) -> Result<(), String> {
    // 1. PDFを読み込む
    let mut doc = Document::load(&path).map_err(|e| e.to_string())?;

    // 2. グループ化
    let mut annots_by_page: HashMap<u32, Vec<AnnotationData>> = HashMap::new();
    for ann in annotations {
        annots_by_page.entry(ann.page as u32).or_default().push(ann);
    }

    // 3. 各ページ処理
    for (page_num, page_id) in doc.get_pages() {
        // このページに紐づくUI上のアノテーションリスト
        let page_annots = annots_by_page.remove(&page_num).unwrap_or_default();
        
        // 最終的にこのページに含まれるべきアノテーションの参照IDリスト
        let mut final_annot_refs = Vec::new();
        
        // ページ情報の取得
        let page_height = get_page_height(&doc, page_id).unwrap_or(842.0);

        for ann in page_annots {
            let pdf_y = page_height - ann.y as f32;
            let font_size = ann.font_size.unwrap_or(14.0);

            // 辞書データの作成
            let mut annot_dict = Dictionary::new();
            annot_dict.set("Type", Object::Name(b"Annot".to_vec()));
            annot_dict.set("Subtype", Object::Name(b"FreeText".to_vec()));
            
            let da_str = format!("0 0 0 rg /Helv {} Tf", font_size);
            annot_dict.set("DA", Object::String(da_str.into_bytes(), StringFormat::Literal));
            
            annot_dict.set("Contents", Object::String(ann.content.clone().into_bytes(), StringFormat::Literal));
            
            annot_dict.set("Rect", Object::Array(vec![
                Object::Real(ann.x as f32),
                Object::Real(pdf_y - (font_size * 1.5)), 
                Object::Real(ann.x as f32 + 200.0),       
                Object::Real(pdf_y)
            ]));

            // 【高速化】IDを持っている(=既存の注釈)なら、そのオブジェクトIDを再利用して上書き
            let object_id = if let Some(id) = ann.object_id {
                // 既存オブジェクトを置換 (doc.objects BTreeMapを直接更新)
                doc.objects.insert(id, Object::Dictionary(annot_dict));
                id
            } else {
                // 新規なら新しいIDを発行して追加
                doc.add_object(annot_dict)
            };

            final_annot_refs.push(Object::Reference(object_id));
        }

        // 4. ページの "Annots" 配列を更新
        // UI上で削除されたアノテーションは final_annot_refs に含まれないため、
        // ページ辞書から参照が消え、実質的に削除される（ファイルサイズ圧縮時に消えるゴミになる）
        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(page_dict) = page_obj.as_dict_mut() {
                if final_annot_refs.is_empty() {
                    page_dict.remove(b"Annots");
                } else {
                    page_dict.set("Annots", Object::Array(final_annot_refs));
                }
            }
        }
    }

    // 保存 (内部構造の整理を行いながら保存)
    doc.save(path).map_err(|e| e.to_string())?;
    Ok(())
}