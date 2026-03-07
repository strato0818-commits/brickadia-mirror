use brdb::{
    BitFlags, BrFsReader, Brick, BrickSize, BrickType, Brz, Collision, Direction, Entity,
    IntoReader, Position, Rotation, SavedBrickColor, World, byte_to_orientation,
};
use eframe::{egui, run_native, App, NativeOptions};
use rfd::{FileDialog, MessageDialog, MessageLevel};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use brdb::schema::{BrdbSchemaGlobalData, BrdbValue, ReadBrdbSchema};
use brdb::AsBrdbValue;

const SOURCE_PLUGIN_JS: &str = include_str!("../../omegga.plugin.js");

#[derive(Debug, Error)]
enum SymmetryError {
    #[error("failed to read BRZ: {0}")]
    Read(String),
    #[error("failed to write BRZ: {0}")]
    Write(String),
    #[error("this BRZ contains entities ({0}) and cannot be mirrored by this tool")]
    UnsupportedEntities(usize),
    #[error("no bricks were found in the selected save")]
    EmptySave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn as_flags(self) -> [bool; 3] {
        match self {
            Axis::X => [true, false, false],
            Axis::Y => [false, true, false],
            Axis::Z => [false, false, true],
        }
    }

    fn label(self) -> &'static str {
        match self {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Clone)]
struct LoadedGrid {
    entity: Entity,
    bricks: Vec<Brick>,
}

#[derive(Default)]
struct SymmetryApp {
    input_path: String,
    output_path: String,
    axis: Option<Axis>,
    z_offset: i32,
    status: String,
}

impl SymmetryApp {
    fn paste_from_clipboard(&mut self) {
        match read_brz_from_clipboard() {
            Ok((bytes, name_hint)) => match write_clipboard_input_temp(&bytes, &name_hint) {
                Ok(path) => {
                    self.input_path = path.to_string_lossy().to_string();
                    self.status = format!(
                        "Pasted {} bytes from clipboard into {}",
                        bytes.len(),
                        self.input_path
                    );
                }
                Err(err) => {
                    self.status = format!("Error: {err}");
                }
            },
            Err(err) => {
                self.status = format!("Error: {err}");
            }
        }
    }

    fn run_current_symmetry(&mut self) {
        let result = run_symmetry(
            Path::new(self.input_path.trim()),
            Path::new(self.output_path.trim()),
            self.axis.expect("axis checked"),
            self.z_offset,
        );

        match result {
            Ok(count) => {
                self.status = format!(
                    "Done: mirrored {} bricks on {} axis",
                    count,
                    self.axis.expect("axis checked").label()
                );
                match copy_output_to_clipboard(Path::new(self.output_path.trim())) {
                    Ok(copy_msg) => {
                        self.status = format!("{}; {}", self.status, copy_msg);
                    }
                    Err(err) => {
                        self.status = format!("{}; clipboard copy failed: {}", self.status, err);
                    }
                }
            }
            Err(e) => {
                self.status = format!("Error: {e}");
                let _ = MessageDialog::new()
                    .set_level(MessageLevel::Error)
                    .set_title("Symmetry failed")
                    .set_description(&self.status)
                    .show();
            }
        }
    }
}

impl App for SymmetryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.input_path.trim().is_empty() {
            if let Some(axis) = self.axis {
                let generated = output_path_for_axis(Path::new(self.input_path.trim()), axis);
                self.output_path = generated.to_string_lossy().to_string();
            }
        } else {
            self.output_path.clear();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("BRZ Symmetry");
            ui.label("Load a .brz, mirror it on X/Y/Z, and export a new .brz.");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Input");
                ui.text_edit_singleline(&mut self.input_path);
                if ui.button("Browse").clicked() {
                    if let Some(path) = FileDialog::new().add_filter("BRZ", &["brz"]).pick_file() {
                        self.input_path = path.to_string_lossy().to_string();
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("Output");
                ui.add_enabled(false, egui::TextEdit::singleline(&mut self.output_path));
            });

            ui.horizontal(|ui| {
                ui.label("Axis");
                for axis in [Axis::X, Axis::Y, Axis::Z] {
                    ui.radio_value(&mut self.axis, Some(axis), axis.label());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Z Offset");
                ui.add(egui::DragValue::new(&mut self.z_offset).speed(1));
            });

            let can_run = !self.input_path.trim().is_empty()
                && !self.output_path.trim().is_empty()
                && self.axis.is_some();

            if ui
                .add_enabled(can_run, egui::Button::new("Apply Symmetry + Copy to Clipboard"))
                .clicked()
            {
                self.run_current_symmetry();
            }

            ui.horizontal(|ui| {
                if ui.button("Paste BRZ from Clipboard").clicked() {
                    self.paste_from_clipboard();
                }
            });

            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }
}

fn run_symmetry(
    input: &Path,
    output: &Path,
    axis: Axis,
    z_offset: i32,
) -> Result<usize, SymmetryError> {
    let reader = Brz::open(input)
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .into_reader();

    if let Some(entity_count) = count_entities(&reader)? {
        return Err(SymmetryError::UnsupportedEntities(entity_count));
    }

    let global_data = reader
        .global_data()
        .map_err(|e| SymmetryError::Read(e.to_string()))?;

    let mut world = World::new();

    let mut main_bricks = read_grid_bricks(&reader, &global_data, 1)?;

    let mut grid_entities: HashMap<usize, Entity> = HashMap::new();
    if let Ok(chunks) = reader.entity_chunk_index() {
        for chunk in chunks {
            for entity in reader
                .entity_chunk(chunk)
                .map_err(|e| SymmetryError::Read(e.to_string()))?
            {
                if entity.is_brick_grid() {
                    if let Some(id) = entity.id {
                        grid_entities.insert(id, entity);
                    }
                } else {
                    world.add_entity(entity);
                }
            }
        }
    }

    let mut grids = Vec::new();
    for (grid_id, entity) in grid_entities {
        let bricks = read_grid_bricks(&reader, &global_data, grid_id)?;
        grids.push(LoadedGrid { entity, bricks });
    }

    let bounds = compute_world_bounds(&main_bricks, &grids).ok_or(SymmetryError::EmptySave)?;
    let axis_flags = axis.as_flags();

    for brick in &mut main_bricks {
        mirror_brick(brick, Position::new(0, 0, 0), bounds, axis_flags);
    }

    for grid in &mut grids {
        let origin = entity_origin_as_position(&grid.entity);
        for brick in &mut grid.bricks {
            mirror_brick(brick, origin, bounds, axis_flags);
        }
    }

    let post_bounds =
        compute_world_bounds(&main_bricks, &grids).ok_or(SymmetryError::EmptySave)?;
    recenter_to_zero(&mut main_bricks, &mut grids, post_bounds);
    let recentered_bounds =
        compute_world_bounds(&main_bricks, &grids).ok_or(SymmetryError::EmptySave)?;
    lift_to_bottom_z_zero(&mut main_bricks, &mut grids, recentered_bounds, z_offset);

    let count = main_bricks.len() + grids.iter().map(|g| g.bricks.len()).sum::<usize>();

    world.add_bricks(main_bricks);
    for grid in grids {
        world.add_brick_grid(grid.entity, grid.bricks);
    }

    world
        .write_brz(output)
        .map_err(|e| SymmetryError::Write(e.to_string()))?;

    Ok(count)
}

fn count_entities(reader: &brdb::BrReader<impl BrFsReader>) -> Result<Option<usize>, SymmetryError> {
    let Ok(chunks) = reader.entity_chunk_index() else {
        return Ok(None);
    };

    let mut count = 0usize;
    for chunk in chunks {
        let entities = reader
            .entity_chunk(chunk)
            .map_err(|e| SymmetryError::Read(e.to_string()))?;
        count += entities.len();
    }

    if count > 0 {
        Ok(Some(count))
    } else {
        Ok(None)
    }
}

fn read_grid_bricks(
    reader: &brdb::BrReader<impl brdb::BrFsReader>,
    global_data: &std::sync::Arc<brdb::schema::BrdbSchemaGlobalData>,
    grid_id: usize,
) -> Result<Vec<Brick>, SymmetryError> {
    let mut out = Vec::new();
    let chunks = reader
        .brick_chunk_index(grid_id)
        .map_err(|e| SymmetryError::Read(e.to_string()))?;

    for chunk in chunks {
        match reader.brick_chunk_soa(grid_id, chunk.index) {
            Ok(soa) => {
                for brick in soa.iter_bricks(chunk.index, global_data.clone()) {
                    out.push(brick.map_err(|e| SymmetryError::Read(e.to_string()))?);
                }
            }
            Err(_) => {
                let mut compat =
                    read_grid_chunk_bricks_compat(reader, global_data, grid_id, chunk.index)?;
                out.append(&mut compat);
            }
        }
    }

    Ok(out)
}

fn recenter_to_zero(main: &mut [Brick], grids: &mut [LoadedGrid], bounds: Bounds) {
    let center_x = ((bounds.min_x + bounds.max_x) as f64 / 2.0).round() as i32;
    let center_y = ((bounds.min_y + bounds.max_y) as f64 / 2.0).round() as i32;
    let center_z = ((bounds.min_z + bounds.max_z) as f64 / 2.0).round() as i32;
    let shift_x = -center_x;
    let shift_y = -center_y;
    let shift_z = -center_z;

    for brick in main {
        brick.position.x += shift_x;
        brick.position.y += shift_y;
        brick.position.z += shift_z;
    }

    for grid in grids {
        grid.entity.location.x += shift_x as f32;
        grid.entity.location.y += shift_y as f32;
        grid.entity.location.z += shift_z as f32;
    }
}

fn lift_to_bottom_z_zero(
    main: &mut [Brick],
    grids: &mut [LoadedGrid],
    bounds: Bounds,
    z_offset: i32,
) {
    let shift_z = -bounds.min_z + z_offset;
    if shift_z == 0 {
        return;
    }

    for brick in main {
        brick.position.z += shift_z;
    }

    for grid in grids {
        grid.entity.location.z += shift_z as f32;
    }
}

fn read_grid_chunk_bricks_compat(
    reader: &brdb::BrReader<impl BrFsReader>,
    global_data: &std::sync::Arc<BrdbSchemaGlobalData>,
    grid_id: usize,
    chunk: brdb::ChunkIndex,
) -> Result<Vec<Brick>, SymmetryError> {
    let path = format!("World/0/Bricks/Grids/{grid_id}/Chunks/{chunk}.mps");
    let found = reader
        .find_file_by_path(&path)
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .ok_or_else(|| SymmetryError::Read(format!("missing file: {path}")))?;

    let schema = reader
        .bricks_schema_rev(found.created_at)
        .map_err(|e| SymmetryError::Read(e.to_string()))?;
    let data = reader
        .find_blob(found.blob_id)
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .read()
        .map_err(|e| SymmetryError::Read(e.to_string()))?;

    let mps = data
        .as_slice()
        .read_brdb(&schema, "BRSavedBrickChunkSoA")
        .map_err(|e| SymmetryError::Read(e.to_string()))?;

    decode_legacy_chunk(&mps, chunk, global_data)
}

fn decode_legacy_chunk(
    mps: &BrdbValue,
    chunk: brdb::ChunkIndex,
    global_data: &std::sync::Arc<BrdbSchemaGlobalData>,
) -> Result<Vec<Brick>, SymmetryError> {
    let procedural_brick_starting_index = mps
        .prop("ProceduralBrickStartingIndex")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .as_brdb_u32()
        .map_err(|e| SymmetryError::Read(e.to_string()))? as usize;
    let brick_size_counters: Vec<brdb::BrickSizeCounter> = mps
        .prop("BrickSizeCounters")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
    let brick_sizes: Vec<BrickSize> = mps
        .prop("BrickSizes")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
    let brick_type_indices: Vec<u32> = mps
        .prop("BrickTypeIndices")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
    let owner_indices: Vec<u32> = mps
        .prop("OwnerIndices")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
    let relative_positions: Vec<brdb::RelativePosition> = mps
        .prop("RelativePositions")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
    let orientations: Vec<u8> = mps
        .prop("Orientations")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
    let material_indices: Vec<u8> = mps
        .prop("MaterialIndices")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
    let colors_and_alphas: Vec<SavedBrickColor> = mps
        .prop("ColorsAndAlphas")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;

    let collision_flags_player: BitFlags = mps
        .prop("CollisionFlags_Player")
        .map_err(|e| SymmetryError::Read(e.to_string()))?
        .try_into()
        .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
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
                    .map_err(|e| SymmetryError::Read(e.to_string()))?,
            )
        } else {
            let size_index = ty_index.saturating_sub(procedural_brick_starting_index);
            let (size, asset_index) = proc_brick_sizes.get(size_index).ok_or_else(|| {
                SymmetryError::Read(format!("invalid procedural size index {size_index}"))
            })?;
            BrickType::Procedural {
                asset: global_data
                    .procedural_brick_asset_by_index(*asset_index as usize)
                    .map_err(|e| SymmetryError::Read(e.to_string()))?,
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
            collision: Collision {
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
                .map_err(|e| SymmetryError::Read(e.to_string()))?,
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
) -> Result<BitFlags, SymmetryError> {
    if mps.contains_key(field) {
        let flags: BitFlags = mps
            .prop(field)
            .map_err(|e| SymmetryError::Read(e.to_string()))?
            .try_into()
            .map_err(|e: brdb::BrdbSchemaError| SymmetryError::Read(e.to_string()))?;
        Ok(flags)
    } else {
        Ok(fallback.clone())
    }
}

fn compute_world_bounds(main: &[Brick], grids: &[LoadedGrid]) -> Option<Bounds> {
    let mut bounds = Bounds::new();
    let mut found = false;

    for brick in main {
        bounds.include(brick.position);
        found = true;
    }

    for grid in grids {
        let origin = entity_origin_as_position(&grid.entity);
        for brick in &grid.bricks {
            bounds.include(Position::new(
                brick.position.x + origin.x,
                brick.position.y + origin.y,
                brick.position.z + origin.z,
            ));
            found = true;
        }
    }

    if found { Some(bounds) } else { None }
}

fn mirror_brick(brick: &mut Brick, origin: Position, bounds: Bounds, axis: [bool; 3]) {
    let mut world_pos = Position::new(
        brick.position.x + origin.x,
        brick.position.y + origin.y,
        brick.position.z + origin.z,
    );

    if axis[0] {
        world_pos.x = bounds.min_x + bounds.max_x - world_pos.x;
    }
    if axis[1] {
        world_pos.y = bounds.min_y + bounds.max_y - world_pos.y;
    }
    if axis[2] {
        world_pos.z = bounds.min_z + bounds.max_z - world_pos.z;
    }

    brick.position = Position::new(
        world_pos.x - origin.x,
        world_pos.y - origin.y,
        world_pos.z - origin.z,
    );

    let original_asset = brick.asset.asset().as_ref().to_string();
    let mirrored_asset = mirror_asset_name(&original_asset).to_string();
    let (new_direction, new_rotation, swap_xy) = convert_direction(
        brick.direction,
        brick.rotation,
        axis,
        rotation_type(&mirrored_asset),
    );

    brick.direction = new_direction;
    brick.rotation = new_rotation;

    if swap_xy {
        if let BrickType::Procedural { size, .. } = &mut brick.asset {
            *size = BrickSize {
                x: size.y,
                y: size.x,
                z: size.z,
            };
        }
    }

    match &mut brick.asset {
        BrickType::Basic(asset) => *asset = mirrored_asset.into(),
        BrickType::Procedural { asset, .. } => *asset = mirrored_asset.into(),
    }
}

fn entity_origin_as_position(entity: &Entity) -> Position {
    Position::new(
        entity.location.x.round() as i32,
        entity.location.y.round() as i32,
        entity.location.z.round() as i32,
    )
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

fn parse_rotation_types() -> HashMap<String, u8> {
    let mut map = HashMap::new();
    let mut in_block = false;

    for line in SOURCE_PLUGIN_JS.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("const rotationTypes = {") {
            in_block = true;
            continue;
        }
        if in_block && trimmed == "};" {
            break;
        }
        if !in_block || trimmed.starts_with("//") {
            continue;
        }

        let Some((k, v)) = trimmed.split_once(':') else {
            continue;
        };
        let key = k.trim().trim_matches('"').trim_matches('\'');
        let value = v
            .trim()
            .trim_end_matches(',')
            .split_whitespace()
            .next()
            .unwrap_or_default();

        if let Ok(parsed) = value.parse::<u8>() {
            map.insert(key.to_string(), parsed);
        }
    }

    map
}

fn parse_mirror_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut in_block = false;

    for line in SOURCE_PLUGIN_JS.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("const mirrorMap = {") {
            in_block = true;
            continue;
        }
        if in_block && trimmed == "}" {
            break;
        }
        if !in_block || trimmed.starts_with("//") {
            continue;
        }

        let Some((k, v)) = trimmed.split_once(':') else {
            continue;
        };
        let key = k.trim().trim_matches('"').trim_matches('\'');
        let value = v
            .trim()
            .trim_end_matches(',')
            .trim_matches('"')
            .trim_matches('\'');

        if !key.is_empty() && !value.is_empty() {
            map.insert(key.to_string(), value.to_string());
        }
    }

    map
}

fn rotation_type(asset_name: &str) -> u8 {
    static ROTATION_TYPES: OnceLock<HashMap<String, u8>> = OnceLock::new();
    ROTATION_TYPES
        .get_or_init(parse_rotation_types)
        .get(asset_name)
        .copied()
        .unwrap_or(1)
}

fn mirror_asset_name(asset_name: &str) -> &str {
    static MIRROR_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    if let Some(mapped) = MIRROR_MAP.get_or_init(parse_mirror_map).get(asset_name) {
        mapped.as_str()
    } else {
        asset_name
    }
}

fn sanitize_brz_name(name_hint: &str) -> String {
    let mut out = String::with_capacity(name_hint.len().max(12));
    for ch in name_hint.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        out = "clipboard.brz".to_string();
    }

    if !out.to_ascii_lowercase().ends_with(".brz") {
        out.push_str(".brz");
    }

    out
}

fn clipboard_temp_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("brz-symmetry-clipboard");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp directory: {e}"))?;
    Ok(dir)
}

fn write_clipboard_input_temp(bytes: &[u8], name_hint: &str) -> Result<PathBuf, String> {
    let mut path = clipboard_temp_dir()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system time error: {e}"))?
        .as_millis();
    path.push(format!("input-{}-{}", stamp, sanitize_brz_name(name_hint)));
    fs::write(&path, bytes).map_err(|e| format!("failed to write clipboard input file: {e}"))?;
    Ok(path)
}

fn copy_output_to_clipboard(output: &Path) -> Result<String, String> {
    let bytes = fs::read(output).map_err(|e| format!("failed to read output file: {e}"))?;
    let name = output
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output.brz");
    write_brz_to_clipboard(name, &bytes)?;
    Ok(format!(
        "Copied {} ({} bytes) to clipboard",
        output.display(),
        bytes.len()
    ))
}

#[cfg(windows)]
fn read_brz_from_clipboard() -> Result<(Vec<u8>, String), String> {
    use clipboard_win::{formats, get_clipboard, register_format};

    let custom_formats = [
        "application/x-brz",
        "application/octet-stream",
        "BRZ",
        "BrickadiaBRZ",
    ];

    for name in custom_formats {
        if let Some(code) = register_format(name) {
            if let Ok(bytes) = get_clipboard(formats::RawData(code.get())) {
                if !bytes.is_empty() {
                    return Ok((bytes, format!("clipboard-raw-{}.brz", name.replace('/', "-"))));
                }
            }
        }
    }

    if let Ok(paths) = get_clipboard::<Vec<PathBuf>, _>(formats::FileList) {
        for path in paths {
            let is_brz = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("brz"))
                .unwrap_or(false);
            if !is_brz {
                continue;
            }

            let bytes = fs::read(&path)
                .map_err(|e| format!("failed to read clipboard file {}: {e}", path.display()))?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("clipboard.brz")
                .to_string();
            return Ok((bytes, name));
        }
    }

    if let Ok(text) = get_clipboard::<String, _>(formats::Unicode) {
        let maybe_path = PathBuf::from(text.trim().trim_matches('"'));
        let is_brz = maybe_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("brz"))
            .unwrap_or(false);
        if is_brz && maybe_path.is_file() {
            let bytes = fs::read(&maybe_path).map_err(|e| {
                format!(
                    "failed to read clipboard path {}: {e}",
                    maybe_path.display()
                )
            })?;
            let name = maybe_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("clipboard.brz")
                .to_string();
            return Ok((bytes, name));
        }
    }

    Err("no BRZ bytes or BRZ file path found on clipboard".to_string())
}

#[cfg(not(windows))]
fn read_brz_from_clipboard() -> Result<(Vec<u8>, String), String> {
    Err("clipboard BRZ integration is only implemented on Windows".to_string())
}

#[cfg(windows)]
fn write_brz_to_clipboard(name_hint: &str, bytes: &[u8]) -> Result<(), String> {
    use clipboard_win::{register_format, Clipboard};
    use clipboard_win::raw;

    let mut temp_path = clipboard_temp_dir()?;
    temp_path.push(format!("copied-{}", sanitize_brz_name(name_hint)));
    fs::write(&temp_path, bytes).map_err(|e| format!("failed to write temp clipboard output: {e}"))?;
    let temp_path_str = temp_path.to_string_lossy().into_owned();

    let _clip = Clipboard::new_attempts(10).map_err(|e| format!("clipboard open failed: {e}"))?;
    raw::empty().map_err(|e| format!("clipboard clear failed: {e}"))?;

    raw::set_file_list(&[temp_path_str.as_str()])
        .map_err(|e| format!("failed to write clipboard file list: {e}"))?;

    for name in ["application/x-brz", "application/octet-stream", "BRZ"] {
        if let Some(code) = register_format(name) {
            let _ = raw::set_without_clear(code.get(), bytes);
        }
    }

    Ok(())
}

#[cfg(not(windows))]
fn write_brz_to_clipboard(_name_hint: &str, _bytes: &[u8]) -> Result<(), String> {
    Err("clipboard BRZ integration is only implemented on Windows".to_string())
}

fn main() {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([700.0, 220.0]),
        ..Default::default()
    };

    let _ = run_native(
        "BRZ Symmetry",
        options,
        Box::new(|_cc| {
            Ok(Box::new(SymmetryApp {
                input_path: String::new(),
                output_path: default_output_path(),
                axis: Some(Axis::X),
                z_offset: 6,
                status: String::new(),
            }))
        }),
    );
}

fn default_output_path() -> String {
    let fallback = "mirrored.brz".to_string();
    let cwd = std::env::current_dir().ok();
    let Some(cwd) = cwd else {
        return fallback;
    };
    cwd.join("mirrored.brz").to_string_lossy().to_string()
}

fn output_path_for_axis(input: &Path, axis: Axis) -> PathBuf {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("mirrored");
    let suffix = axis.label().to_lowercase();
    parent.join(format!("{stem}-{suffix}.brz"))
}
