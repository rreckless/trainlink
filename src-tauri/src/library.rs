//! The track library: manufacturer pieces, from the JSON files built into the app

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::{Map, Value};

/// Each file holds one manufacturer and gauge
const FILES: [&str; 2] = [
    include_str!("../library/custom_n.json"),
    include_str!("../library/peco_n.json"),
];

/// Fields copied from the file into each piece. Geometry fields are present only where they apply
const PIECE_FIELDS: [&str; 20] = [
    "part_number", "series", "system", "name", "type", "length", "radius", "angle", "hand",
    "inner_radius", "outer_radius", "slip_geometry", "curve_geometry", "diverge_geometry",
    "three_way_geometry", "placeable", "angle_meaning", "verified", "source", "note",
];

pub struct Library {
    by_id: BTreeMap<String, Value>,
}

/// Piece ids are "<manufacturer>-<gauge>-<part number>", lowercased, with spaces as dashes
pub fn piece_id(manufacturer: &str, gauge: &str, part_number: &str) -> String {
    format!("{manufacturer}-{gauge}-{part_number}").to_lowercase().replace(' ', "-")
}

fn build() -> Result<Library, String> {
    let mut by_id = BTreeMap::new();
    for text in FILES {
        let data: Value = serde_json::from_str(text).map_err(|error| format!("Track library file: {error}"))?;
        let manufacturer = data["manufacturer"].as_str().unwrap_or_default();
        let gauge = data["gauge"].as_str().unwrap_or_default();
        let file_source = data["source"].as_str().unwrap_or_default();
        for raw in data["pieces"].as_array().into_iter().flatten() {
            let part_number = raw["part_number"].as_str().unwrap_or_default();
            let id = piece_id(manufacturer, gauge, part_number);
            if by_id.contains_key(&id) {
                return Err(format!("Duplicate track library piece: {id}"));
            }
            let mut piece = Map::new();
            for field in PIECE_FIELDS {
                if let Some(value) = raw.get(field) {
                    piece.insert(field.into(), value.clone());
                }
            }
            piece.insert("id".into(), Value::String(id.clone()));
            piece.insert("manufacturer".into(), Value::String(manufacturer.into()));
            piece.insert("gauge".into(), Value::String(gauge.into()));
            piece.entry("verified").or_insert(Value::Bool(false));
            piece.entry("source").or_insert(Value::String(file_source.into()));
            by_id.insert(id, Value::Object(piece));
        }
    }
    Ok(Library { by_id })
}

/// The library, loaded once. The data is built into the app, so a failure here is a build error
pub fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| build().unwrap_or_else(|error| panic!("{error}")))
}

impl Library {
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.by_id.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// Pieces that match each filter given, sorted by manufacturer, system, then part number
    pub fn list(&self, system: Option<&str>, gauge: Option<&str>, manufacturer: Option<&str>) -> Vec<&Value> {
        let matches = |piece: &&Value, field: &str, wanted: Option<&str>| {
            wanted.is_none_or(|value| piece[field].as_str() == Some(value))
        };
        let mut pieces: Vec<&Value> = self
            .by_id
            .values()
            .filter(|piece| matches(piece, "system", system))
            .filter(|piece| matches(piece, "gauge", gauge))
            .filter(|piece| matches(piece, "manufacturer", manufacturer))
            .collect();
        pieces.sort_by_key(|piece| {
            (
                piece["manufacturer"].as_str().unwrap_or_default().to_string(),
                piece["system"].as_str().unwrap_or_default().to_string(),
                piece["part_number"].as_str().unwrap_or_default().to_string(),
            )
        });
        pieces
    }
}
