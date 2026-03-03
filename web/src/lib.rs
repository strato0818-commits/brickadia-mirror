use brdb::schema::{BrdbSchemaGlobalData, BrdbValue, ReadBrdbSchema};
use brdb::{
    byte_to_orientation, AsBrdbValue, BitFlags, BrFsReader, Brick, BrickSize, BrickType, Brz,
    Direction, IntoReader, Position, Rotation, SavedBrickColor, World,
};
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn parse(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            _ => None,
        }
    }

    fn as_flags(self) -> [bool; 3] {
        match self {
            Axis::X => [true, false, false],
            Axis::Y => [false, true, false],
            Axis::Z => [false, false, true],
        }
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
}

impl Bounds {
    fn new() -> Self {
        Self {
            min_x: i32::MAX,
            min_y: i32::MAX,
            min_z: i32::MAX,
            max_x: i32::MIN,
            max_y: i32::MIN,
            max_z: i32::MIN,
        }
    }

    fn include(&mut self, p: Position) {
        self.min_x = self.min_x.min(p.x);
        self.min_y = self.min_y.min(p.y);
        self.min_z = self.min_z.min(p.z);
        self.max_x = self.max_x.max(p.x);
        self.max_y = self.max_y.max(p.y);
        self.max_z = self.max_z.max(p.z);
    }
}

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
    let axis = Axis::parse(axis).ok_or_else(|| JsValue::from_str("axis must be x, y, or z"))?;

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

    let global_data = reader
        .global_data()
        .map_err(|e| JsValue::from_str(&format!("failed to read global data: {e}")))?;

    let mut bricks = read_main_bricks(&reader, global_data)?;
    let bounds = compute_bounds(&bricks)
        .ok_or_else(|| JsValue::from_str("no bricks were found in the selected save"))?;
    let axis_flags = axis.as_flags();

    for brick in &mut bricks {
        mirror_brick(brick, bounds, axis_flags);
    }

    recenter_and_lift(&mut bricks)?;

    let mut world = World::new();
    world.add_bricks(bricks);

    let data = world
        .to_brz_vec()
        .map_err(|e| JsValue::from_str(&format!("failed to write BRZ bytes: {e}")))?;

    Ok(data)
}

fn read_main_bricks(
    reader: &brdb::BrReader<impl brdb::BrFsReader>,
    global_data: std::sync::Arc<BrdbSchemaGlobalData>,
) -> Result<Vec<Brick>, JsValue> {
    let chunks = reader
        .brick_chunk_index(1)
        .map_err(|e| JsValue::from_str(&format!("failed to read brick chunks: {e}")))?;

    let mut out = Vec::new();
    for chunk in chunks {
        match reader.brick_chunk_soa(1, chunk.index) {
            Ok(soa) => {
                for brick in soa.iter_bricks(chunk.index, global_data.clone()) {
                    out.push(
                        brick.map_err(|e| {
                            JsValue::from_str(&format!("failed to decode brick: {e}"))
                        })?,
                    );
                }
            }
            Err(_) => {
                let mut compat = read_grid_chunk_bricks_compat(reader, &global_data, 1, chunk.index)?;
                out.append(&mut compat);
            }
        }
    }

    Ok(out)
}

fn read_grid_chunk_bricks_compat(
    reader: &brdb::BrReader<impl BrFsReader>,
    global_data: &std::sync::Arc<BrdbSchemaGlobalData>,
    grid_id: usize,
    chunk: brdb::ChunkIndex,
) -> Result<Vec<Brick>, JsValue> {
    let path = format!("World/0/Bricks/Grids/{grid_id}/Chunks/{chunk}.mps");
    let found = reader
        .find_file_by_path(&path)
        .map_err(|e| JsValue::from_str(&format!("failed to find legacy chunk path: {e}")))?
        .ok_or_else(|| JsValue::from_str(&format!("missing file: {path}")))?;

    let schema = reader
        .bricks_schema_rev(found.created_at)
        .map_err(|e| JsValue::from_str(&format!("failed to load brick schema: {e}")))?;
    let data = reader
        .find_blob(found.blob_id)
        .map_err(|e| JsValue::from_str(&format!("failed to find chunk blob: {e}")))?
        .read()
        .map_err(|e| JsValue::from_str(&format!("failed to read chunk blob: {e}")))?;

    let mps = data
        .as_slice()
        .read_brdb(&schema, "BRSavedBrickChunkSoA")
        .map_err(|e| JsValue::from_str(&format!("failed to decode legacy chunk: {e}")))?;

    decode_legacy_chunk(&mps, chunk, global_data)
}

fn decode_legacy_chunk(
    mps: &BrdbValue,
    chunk: brdb::ChunkIndex,
    global_data: &std::sync::Arc<BrdbSchemaGlobalData>,
) -> Result<Vec<Brick>, JsValue> {
    let procedural_brick_starting_index = mps
        .prop("ProceduralBrickStartingIndex")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing ProceduralBrickStartingIndex: {e}")))?
        .as_brdb_u32()
        .map_err(|e| JsValue::from_str(&format!("invalid ProceduralBrickStartingIndex: {e}")))?
        as usize;
    let brick_size_counters: Vec<brdb::BrickSizeCounter> = mps
        .prop("BrickSizeCounters")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing BrickSizeCounters: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid BrickSizeCounters: {e}")))?;
    let brick_sizes: Vec<BrickSize> = mps
        .prop("BrickSizes")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing BrickSizes: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid BrickSizes: {e}")))?;
    let brick_type_indices: Vec<u32> = mps
        .prop("BrickTypeIndices")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing BrickTypeIndices: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid BrickTypeIndices: {e}")))?;
    let owner_indices: Vec<u32> = mps
        .prop("OwnerIndices")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing OwnerIndices: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid OwnerIndices: {e}")))?;
    let relative_positions: Vec<brdb::RelativePosition> = mps
        .prop("RelativePositions")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing RelativePositions: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid RelativePositions: {e}")))?;
    let orientations: Vec<u8> = mps
        .prop("Orientations")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing Orientations: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid Orientations: {e}")))?;
    let material_indices: Vec<u8> = mps
        .prop("MaterialIndices")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing MaterialIndices: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid MaterialIndices: {e}")))?;
    let colors_and_alphas: Vec<SavedBrickColor> = mps
        .prop("ColorsAndAlphas")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing ColorsAndAlphas: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid ColorsAndAlphas: {e}")))?;

    let collision_flags_player: BitFlags = mps
        .prop("CollisionFlags_Player")
        .map_err(|e| JsValue::from_str(&format!("legacy chunk missing CollisionFlags_Player: {e}")))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid CollisionFlags_Player: {e}")))?;
    let collision_flags_player1 =
        parse_optional_flags(mps, "CollisionFlags_Player1", &collision_flags_player)?;
    let collision_flags_player2 =
        parse_optional_flags(mps, "CollisionFlags_Player2", &collision_flags_player)?;
    let collision_flags_player3 =
        parse_optional_flags(mps, "CollisionFlags_Player3", &collision_flags_player)?;
    let collision_flags_weapon =
        parse_optional_flags(mps, "CollisionFlags_Weapon", &collision_flags_player)?;
    let collision_flags_interaction =
        parse_optional_flags(mps, "CollisionFlags_Interaction", &collision_flags_weapon)?;
    let collision_flags_tool =
        parse_optional_flags(mps, "CollisionFlags_Tool", &collision_flags_interaction)?;
    let collision_flags_physics =
        parse_optional_flags(mps, "CollisionFlags_Physics", &collision_flags_player)?;
    let visibility_flags = parse_optional_flags(
        mps,
        "VisibilityFlags",
        &BitFlags::new_full(brick_type_indices.len()),
    )?;

    let proc_brick_sizes = brick_sizes
        .iter()
        .copied()
        .zip(
            brick_size_counters
                .iter()
                .flat_map(|c| (0..c.num_sizes).map(|_| c.asset_index)),
        )
        .collect::<Vec<_>>();

    let mut out = Vec::with_capacity(brick_type_indices.len());
    for i in 0..brick_type_indices.len() {
        let ty_index = brick_type_indices[i] as usize;
        let asset = if ty_index < procedural_brick_starting_index {
            BrickType::Basic(
                global_data
                    .basic_brick_asset_by_index(ty_index)
                    .map_err(|e| JsValue::from_str(&format!("invalid basic asset index: {e}")))?,
            )
        } else {
            let size_index = ty_index.saturating_sub(procedural_brick_starting_index);
            let (size, asset_index) = proc_brick_sizes
                .get(size_index)
                .ok_or_else(|| JsValue::from_str(&format!("invalid procedural size index {size_index}")))?;
            BrickType::Procedural {
                asset: global_data
                    .procedural_brick_asset_by_index(*asset_index as usize)
                    .map_err(|e| JsValue::from_str(&format!("invalid procedural asset index: {e}")))?,
                size: *size,
            }
        };

        let position = Position::from_relative(chunk, relative_positions[i]);
        let (direction, rotation) = byte_to_orientation(orientations[i]);
        let color = colors_and_alphas[i];

        out.push(Brick {
            id: None,
            asset,
            owner_index: Some(owner_indices[i] as usize),
            position,
            rotation,
            direction,
            collision: brdb::Collision {
                player: collision_flags_player.get(i),
                player1: Some(collision_flags_player1.get(i)),
                player2: Some(collision_flags_player2.get(i)),
                player3: Some(collision_flags_player3.get(i)),
                weapon: collision_flags_weapon.get(i),
                interact: collision_flags_interaction.get(i),
                tool: collision_flags_tool.get(i),
                physics: collision_flags_physics.get(i),
            },
            visible: visibility_flags.get(i),
            color: color.color(),
            material: global_data
                .material_asset_by_index(material_indices[i] as usize)
                .map_err(|e| JsValue::from_str(&format!("invalid material index: {e}")))?,
            material_intensity: color.a,
            components: Vec::new(),
        });
    }

    Ok(out)
}

fn parse_optional_flags(
    mps: &BrdbValue,
    field: &str,
    fallback: &BitFlags,
) -> Result<BitFlags, JsValue> {
    if mps.contains_key(field) {
        let flags: BitFlags = mps
            .prop(field)
            .map_err(|e| JsValue::from_str(&format!("failed to read {field}: {e}")))?
            .try_into()
            .map_err(|e: brdb::BrdbSchemaError| JsValue::from_str(&format!("invalid {field}: {e}")))?;
        Ok(flags)
    } else {
        Ok(fallback.clone())
    }
}

fn compute_bounds(bricks: &[Brick]) -> Option<Bounds> {
    let mut bounds = Bounds::new();
    let mut found = false;
    for brick in bricks {
        bounds.include(brick.position);
        found = true;
    }
    if found { Some(bounds) } else { None }
}

fn recenter_and_lift(bricks: &mut [Brick]) -> Result<(), JsValue> {
    let bounds =
        compute_bounds(bricks).ok_or_else(|| JsValue::from_str("no bricks were found in save"))?;
    let center_x = ((bounds.min_x + bounds.max_x) as f64 / 2.0).round() as i32;
    let center_y = ((bounds.min_y + bounds.max_y) as f64 / 2.0).round() as i32;
    let shift_x = -center_x;
    let shift_y = -center_y;
    let shift_z = -bounds.min_z;

    for brick in bricks {
        brick.position.x += shift_x;
        brick.position.y += shift_y;
        brick.position.z += shift_z;
    }
    Ok(())
}

fn mirror_brick(brick: &mut Brick, bounds: Bounds, axis: [bool; 3]) {
    let mut pos = brick.position;
    if axis[0] {
        pos.x = bounds.min_x + bounds.max_x - pos.x;
    }
    if axis[1] {
        pos.y = bounds.min_y + bounds.max_y - pos.y;
    }
    if axis[2] {
        pos.z = bounds.min_z + bounds.max_z - pos.z;
    }
    brick.position = pos;

    let (new_direction, new_rotation, swap_xy) =
        convert_direction(brick.direction, brick.rotation, axis, 1);
    brick.direction = new_direction;
    brick.rotation = new_rotation;

    if swap_xy && matches!(brick.asset, BrickType::Procedural { .. }) {
        if let BrickType::Procedural { size, .. } = &mut brick.asset {
            *size = BrickSize {
                x: size.y,
                y: size.x,
                z: size.z,
            };
        }
    }
}

#[derive(Clone, Copy)]
struct AxisRule {
    axis: usize,
    flip: bool,
    turn1: usize,
    turn3: usize,
    direction_flip: usize,
}

const AXIS_MAP: [AxisRule; 6] = [
    AxisRule {
        axis: 0,
        flip: false,
        turn1: 1,
        turn3: 2,
        direction_flip: 1,
    },
    AxisRule {
        axis: 0,
        flip: false,
        turn1: 1,
        turn3: 2,
        direction_flip: 0,
    },
    AxisRule {
        axis: 1,
        flip: false,
        turn1: 0,
        turn3: 2,
        direction_flip: 3,
    },
    AxisRule {
        axis: 1,
        flip: false,
        turn1: 0,
        turn3: 2,
        direction_flip: 2,
    },
    AxisRule {
        axis: 2,
        flip: true,
        turn1: 1,
        turn3: 0,
        direction_flip: 5,
    },
    AxisRule {
        axis: 2,
        flip: true,
        turn1: 1,
        turn3: 0,
        direction_flip: 4,
    },
];

fn convert_direction(
    direction: Direction,
    rotation: Rotation,
    axis: [bool; 3],
    rotation_type: u8,
) -> (Direction, Rotation, bool) {
    let map = AXIS_MAP[direction_to_index(direction)];
    let original_rotation = rotation_to_index(rotation);
    let mut dir = direction_to_index(direction);
    let mut rot = original_rotation;

    if axis[map.axis] {
        dir = map.direction_flip;
        rot = rotate(
            rot,
            match rotation_type {
                1 => {
                    if rot % 2 == 1 {
                        if map.flip { 0 } else { 2 }
                    } else if map.flip {
                        2
                    } else {
                        0
                    }
                }
                2 => {
                    if rot % 2 == 1 {
                        if map.flip { 1 } else { 3 }
                    } else if map.flip {
                        3
                    } else {
                        1
                    }
                }
                3 => {
                    if rot % 2 == 1 {
                        if map.flip { 3 } else { 1 }
                    } else if map.flip {
                        1
                    } else {
                        3
                    }
                }
                4 => {
                    if rot % 2 == 1 {
                        if map.flip { 2 } else { 0 }
                    } else if map.flip {
                        0
                    } else {
                        2
                    }
                }
                _ => 0,
            },
        );
    }

    if axis[map.turn1] {
        rot = rotate(
            rot,
            match rotation_type {
                1 => {
                    if rot % 2 == 1 { 2 } else { 0 }
                }
                2 => {
                    if rot % 2 == 1 { 3 } else { 1 }
                }
                3 => {
                    if rot % 2 == 1 { 1 } else { 3 }
                }
                4 => {
                    if rot % 2 == 1 { 0 } else { 2 }
                }
                _ => 0,
            },
        );
    }

    if axis[map.turn3] {
        rot = rotate(
            rot,
            match rotation_type {
                1 => {
                    if rot % 2 == 1 { 0 } else { 2 }
                }
                2 => {
                    if rot % 2 == 1 { 1 } else { 3 }
                }
                3 => {
                    if rot % 2 == 1 { 3 } else { 1 }
                }
                4 => {
                    if rot % 2 == 1 { 2 } else { 0 }
                }
                _ => 0,
            },
        );
    }

    (
        index_to_direction(dir),
        index_to_rotation(rot),
        (original_rotation + rot) % 2 == 1,
    )
}

fn rotate(rotation: usize, turns: usize) -> usize {
    (rotation + turns) % 4
}

fn direction_to_index(direction: Direction) -> usize {
    match direction {
        Direction::XPositive => 0,
        Direction::XNegative => 1,
        Direction::YPositive => 2,
        Direction::YNegative => 3,
        Direction::ZPositive => 4,
        Direction::ZNegative => 5,
        Direction::MAX => 4,
    }
}

fn index_to_direction(index: usize) -> Direction {
    match index {
        0 => Direction::XPositive,
        1 => Direction::XNegative,
        2 => Direction::YPositive,
        3 => Direction::YNegative,
        4 => Direction::ZPositive,
        _ => Direction::ZNegative,
    }
}

fn rotation_to_index(rotation: Rotation) -> usize {
    match rotation {
        Rotation::Deg0 => 0,
        Rotation::Deg90 => 1,
        Rotation::Deg180 => 2,
        Rotation::Deg270 => 3,
    }
}

fn index_to_rotation(index: usize) -> Rotation {
    match index % 4 {
        0 => Rotation::Deg0,
        1 => Rotation::Deg90,
        2 => Rotation::Deg180,
        _ => Rotation::Deg270,
    }
}
