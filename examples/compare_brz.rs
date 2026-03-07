use brdb::{Brz, IntoReader, Position, Entity};
use std::collections::{HashMap, BTreeMap};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
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

#[derive(Debug)]
struct Summary {
    file_size: u64,
    index_num_files: i32,
    index_num_folders: i32,
    world_file_count: usize,
    world_brick_file_count: usize,
    world_paths_with_sizes: Vec<(String, i32)>,
    all_paths_with_sizes: BTreeMap<String, i32>,
    main_bricks: usize,
    grid_bricks: usize,
    total_bricks: usize,
    entities_total: usize,
    grid_entities: usize,
    bounds: Option<Bounds>,
}

fn summarize(path: &Path) -> Result<Summary, Box<dyn std::error::Error>> {
    let file_size = std::fs::metadata(path)?.len();
    let brz = Brz::open(path)?;
    let index_num_files = brz.index_data.num_files;
    let index_num_folders = brz.index_data.num_folders;
    let (world_file_count, world_brick_file_count, world_paths_with_sizes, all_paths_with_sizes) =
        summarize_index_paths(&brz);
    let reader = brz.into_reader();
    let global_data = reader.global_data()?;

    let mut entities_total = 0usize;
    let mut grid_entities: HashMap<usize, Entity> = HashMap::new();

    if let Ok(chunks) = reader.entity_chunk_index() {
        for chunk in chunks {
            for entity in reader.entity_chunk(chunk)? {
                entities_total += 1;
                if entity.is_brick_grid() {
                    if let Some(id) = entity.id {
                        grid_entities.insert(id, entity);
                    }
                }
            }
        }
    }

    let mut main_bricks = 0usize;
    let mut grid_bricks = 0usize;
    let mut bounds = Bounds::new();
    let mut any = false;

    if let Ok(chunks) = reader.brick_chunk_index(1) {
        for chunk in chunks {
            if let Ok(soa) = reader.brick_chunk_soa(1, chunk.index) {
                for brick in soa.iter_bricks(chunk.index, global_data.clone()) {
                    let b = brick?;
                    main_bricks += 1;
                    bounds.include(b.position);
                    any = true;
                }
            }
        }
    }

    for (grid_id, entity) in grid_entities.iter() {
        if let Ok(chunks) = reader.brick_chunk_index(*grid_id) {
            for chunk in chunks {
                if let Ok(soa) = reader.brick_chunk_soa(*grid_id, chunk.index) {
                    for brick in soa.iter_bricks(chunk.index, global_data.clone()) {
                        let b = brick?;
                        grid_bricks += 1;
                        let p = Position::new(
                            b.position.x + entity.location.x.round() as i32,
                            b.position.y + entity.location.y.round() as i32,
                            b.position.z + entity.location.z.round() as i32,
                        );
                        bounds.include(p);
                        any = true;
                    }
                }
            }
        }
    }

    Ok(Summary {
        file_size,
        index_num_files,
        index_num_folders,
        world_file_count,
        world_brick_file_count,
        world_paths_with_sizes,
        all_paths_with_sizes,
        main_bricks,
        grid_bricks,
        total_bricks: main_bricks + grid_bricks,
        entities_total,
        grid_entities: grid_entities.len(),
        bounds: if any { Some(bounds) } else { None },
    })
}

fn summarize_index_paths(brz: &Brz) -> (usize, usize, Vec<(String, i32)>, BTreeMap<String, i32>) {
    fn folder_path(index: i32, parents: &[i32], names: &[String]) -> String {
        if index < 0 {
            return String::new();
        }
        let mut parts = Vec::new();
        let mut cur = index;
        let mut guard = 0usize;
        while cur >= 0 && guard < 10000 {
            guard += 1;
            let i = cur as usize;
            if i >= names.len() || i >= parents.len() {
                break;
            }
            parts.push(names[i].clone());
            cur = parents[i];
        }
        parts.reverse();
        parts.join("/")
    }

    let mut world_file_count = 0usize;
    let mut world_brick_file_count = 0usize;
    let mut sample = Vec::new();
    let mut all_paths_with_sizes = BTreeMap::new();

    for i in 0..(brz.index_data.num_files as usize) {
        let parent = brz.index_data.file_parent_ids.get(i).copied().unwrap_or(-1);
        let file = brz.index_data.file_names.get(i).cloned().unwrap_or_default();
        let folder = folder_path(
            parent,
            &brz.index_data.folder_parent_ids,
            &brz.index_data.folder_names,
        );
        let full = if folder.is_empty() {
            file
        } else {
            format!("{folder}/{file}")
        };
        let content_id = brz.index_data.file_content_ids.get(i).copied().unwrap_or(-1);
        let size = if content_id >= 0 {
            brz.index_data
                .sizes_uncompressed
                .get(content_id as usize)
                .copied()
                .unwrap_or(-1)
        } else {
            0
        };
        all_paths_with_sizes.insert(full.clone(), size);

        if full.starts_with("World/") {
            world_file_count += 1;
            sample.push((full.clone(), size));
        }

        if full.contains("/Bricks/") {
            world_brick_file_count += 1;
        }
    }

    (world_file_count, world_brick_file_count, sample, all_paths_with_sizes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let a = args.next().expect("usage: compare_brz <a.brz> <b.brz>");
    let b = args.next().expect("usage: compare_brz <a.brz> <b.brz>");

    let sa = summarize(Path::new(&a))?;
    let sb = summarize(Path::new(&b))?;

    println!("A: {}", a);
    println!("  size={} bytes", sa.file_size);
    println!(
        "  index: files={}, folders={}, world_files={}, brick_files={}",
        sa.index_num_files, sa.index_num_folders, sa.world_file_count, sa.world_brick_file_count
    );
    if !sa.world_paths_with_sizes.is_empty() {
        println!("  world paths (uncompressed blob size):");
        for (p, sz) in &sa.world_paths_with_sizes {
            println!("    {} [{}]", p, sz);
        }
    }
    println!("  bricks: main={}, grid={}, total={}", sa.main_bricks, sa.grid_bricks, sa.total_bricks);
    println!("  entities: total={}, grid_entities={}", sa.entities_total, sa.grid_entities);
    match sa.bounds {
        Some(b) => println!("  bounds: x[{}, {}] y[{}, {}] z[{}, {}]", b.min_x,b.max_x,b.min_y,b.max_y,b.min_z,b.max_z),
        None => println!("  bounds: (none)"),
    }

    println!("B: {}", b);
    println!("  size={} bytes", sb.file_size);
    println!(
        "  index: files={}, folders={}, world_files={}, brick_files={}",
        sb.index_num_files, sb.index_num_folders, sb.world_file_count, sb.world_brick_file_count
    );
    if !sb.world_paths_with_sizes.is_empty() {
        println!("  world paths (uncompressed blob size):");
        for (p, sz) in &sb.world_paths_with_sizes {
            println!("    {} [{}]", p, sz);
        }
    }
    println!("  bricks: main={}, grid={}, total={}", sb.main_bricks, sb.grid_bricks, sb.total_bricks);
    println!("  entities: total={}, grid_entities={}", sb.entities_total, sb.grid_entities);
    match sb.bounds {
        Some(b) => println!("  bounds: x[{}, {}] y[{}, {}] z[{}, {}]", b.min_x,b.max_x,b.min_y,b.max_y,b.min_z,b.max_z),
        None => println!("  bounds: (none)"),
    }

    println!("Delta B-A:");
    println!("  size: {}", sb.file_size as i64 - sa.file_size as i64);
    println!("  total bricks: {}", sb.total_bricks as i64 - sa.total_bricks as i64);
    println!("  entities: {}", sb.entities_total as i64 - sa.entities_total as i64);

    let mut only_a = Vec::new();
    let mut only_b = Vec::new();
    for (k, v) in &sa.all_paths_with_sizes {
        if !sb.all_paths_with_sizes.contains_key(k) {
            only_a.push((k, v));
        }
    }
    for (k, v) in &sb.all_paths_with_sizes {
        if !sa.all_paths_with_sizes.contains_key(k) {
            only_b.push((k, v));
        }
    }

    if !only_a.is_empty() {
        println!("  paths only in A:");
        for (k, v) in only_a {
            println!("    {} [{}]", k, v);
        }
    }
    if !only_b.is_empty() {
        println!("  paths only in B:");
        for (k, v) in only_b {
            println!("    {} [{}]", k, v);
        }
    }

    println!("  shared path size deltas (B-A):");
    for (k, va) in &sa.all_paths_with_sizes {
        if let Some(vb) = sb.all_paths_with_sizes.get(k) {
            let d = *vb - *va;
            if d != 0 {
                println!("    {}: {} -> {} (delta {})", k, va, vb, d);
            }
        }
    }

    Ok(())
}
