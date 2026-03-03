use brdb::{Brz, IntoReader};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn validate_brz(input: &[u8]) -> Result<String, JsValue> {
    let reader = Brz::read_slice(input)
        .map_err(|e| JsValue::from_str(&format!("failed to read BRZ: {e}")))?
        .into_reader();

    let global_data = reader
        .global_data()
        .map_err(|e| JsValue::from_str(&format!("failed to read global data: {e}")))?;

    let mut entity_count = 0usize;
    if let Ok(chunks) = reader.entity_chunk_index() {
        for chunk in chunks {
            entity_count += reader
                .entity_chunk(chunk)
                .map_err(|e| JsValue::from_str(&format!("failed to read entity chunk: {e}")))?
                .len();
        }
    }

    Ok(format!(
        "ok: basic_assets={}, entities={}",
        global_data.basic_brick_asset_names.len(),
        entity_count
    ))
}

#[wasm_bindgen]
pub fn process_brz(input: &[u8], axis: &str) -> Result<Vec<u8>, JsValue> {
    if !matches!(axis.to_ascii_lowercase().as_str(), "x" | "y" | "z") {
        return Err(JsValue::from_str("axis must be x, y, or z"));
    }

    let brz = Brz::read_slice(input)
        .map_err(|e| JsValue::from_str(&format!("failed to read BRZ: {e}")))?;

    let reader = brz.into_reader();

    let mut entity_count = 0usize;
    if let Ok(chunks) = reader.entity_chunk_index() {
        for chunk in chunks {
            entity_count += reader
                .entity_chunk(chunk)
                .map_err(|e| JsValue::from_str(&format!("failed to read entity chunk: {e}")))?
                .len();
        }
    }

    if entity_count > 0 {
        return Err(JsValue::from_str(&format!(
            "this BRZ contains entities ({entity_count}) and cannot be mirrored by this tool yet"
        )));
    }

    let pending = reader
        .to_pending()
        .map_err(|e| JsValue::from_str(&format!("failed to convert BRZ: {e}")))?;

    let data = pending
        .to_brz_data(Some(14))
        .map_err(|e| JsValue::from_str(&format!("failed to encode BRZ: {e}")))?
        .to_vec(Some(14))
        .map_err(|e| JsValue::from_str(&format!("failed to write BRZ bytes: {e}")))?;

    Ok(data)
}
