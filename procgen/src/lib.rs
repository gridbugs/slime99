use direction::{CardinalDirection, Direction};
use grid_2d::{coord_2d::Axis, Coord, Grid, Size};
use rand::{seq::SliceRandom, Rng};
use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroU32;
use wfc::{overlapping::OverlappingPatterns, retry, wrap, ForbidNothing, RunOwn};

#[rustfmt::skip]
const WFC_INPUT: &[&str] = &[
"................................",
"................................",
"................................",
"#############################...",
"#...........................#...",
"#...........................#...",
"#...........................#...",
"#...........................#...",
"#.............###...........#...",
"#...........###.####........#...",
"#.........###......###......#...",
"#........##..........##.....#...",
"#.......##............#.....#...",
"#.......#.............#.....#...",
"#......##.............#.....#...",
"##....##.............##.....#...",
".#....#..............#......#...",
".#....#..............#......#...",
".#....#.............##......#...",
".#....#.............#.......#...",
".#....#.............#.......#...",
".#....##...........##.......#...",
".#.....#...........#........#...",
".#.....##..........#........#...",
".#......##.........#........#...",
".#.......##........###......#...",
".#........###........########...",
".#..........###.............#...",
".#............##............#...",
".#.............##...........#...",
".#..............#...........#...",
".############################...",
];

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum CellA {
    Closed,
    Open,
}

fn input_grid_from_strs(input: &[&str]) -> Grid<CellA> {
    let width = input[0].len();
    let height = input.len();
    let size = Size::new(width as u32, height as u32);
    let mut grid = Grid::new_clone(size, CellA::Open);
    for (y, row) in input.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let coord = Coord::new(x as i32, y as i32);
            let cell = match ch {
                '.' => CellA::Open,
                '#' => CellA::Closed,
                ch => panic!("unexpected char: {}", ch),
            };
            *grid.get_checked_mut(coord) = cell;
        }
    }
    grid
}

fn wfc_map<R: Rng>(
    input_grid: Grid<CellA>,
    output_size: Size,
    pattern_size: NonZeroU32,
    rng: &mut R,
) -> Grid<CellA> {
    let mut output_grid = Grid::new_clone(output_size, CellA::Open);
    let overlapping_patterns = OverlappingPatterns::new_all_orientations(input_grid, pattern_size);
    let global_stats = overlapping_patterns.global_stats();
    let run = RunOwn::new_wrap_forbid(output_size, &global_stats, wrap::WrapXY, ForbidNothing, rng);
    let wave = run.collapse_retrying(retry::Forever, rng);
    for (coord, wave_cell) in wave.grid().enumerate() {
        let pattern_id = wave_cell
            .chosen_pattern_id()
            .expect("unexpected contradiction");
        let cell = overlapping_patterns.pattern_top_left_value(pattern_id);
        *output_grid.get_checked_mut(coord) = *cell;
    }
    output_grid
}

struct PoolCandidates {
    num: u32,
    grid: Grid<Option<u32>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CellB {
    Wall,
    Floor,
    Pool,
}

impl PoolCandidates {
    fn new(grid: &Grid<CellA>) -> Self {
        let mut candidate_grid: Grid<Option<u32>> = Grid::new_clone(grid.size(), None);
        let mut num_candidates = 0;
        let mut seen = HashSet::new();
        let mut flood_fill_buffer = VecDeque::new();
        for (coord, cell) in grid.enumerate() {
            if let CellA::Open = cell {
                if seen.insert(coord) {
                    flood_fill_buffer.push_back(coord);
                    while let Some(coord) = flood_fill_buffer.pop_front() {
                        *candidate_grid.get_checked_mut(coord) = Some(num_candidates);
                        for direction in CardinalDirection::all() {
                            let neighbour_coord = coord + direction.coord();
                            if let Some(CellA::Open) = grid.get(neighbour_coord) {
                                if seen.insert(neighbour_coord) {
                                    flood_fill_buffer.push_back(neighbour_coord);
                                }
                            }
                        }
                    }
                    num_candidates += 1;
                }
            }
        }
        Self {
            num: num_candidates,
            grid: candidate_grid,
        }
    }
    fn shrink_candidate(&mut self, candidate: u32) {
        let mut to_remove = Vec::new();
        for (coord, cell) in self.grid.enumerate() {
            if *cell == Some(candidate) {
                if Direction::all().map(|d| d.coord() + coord).any(|c| {
                    if let Some(cell) = self.grid.get(c) {
                        cell.is_none()
                    } else {
                        false
                    }
                }) {
                    to_remove.push(coord);
                }
            }
        }
        for coord in to_remove {
            *self.grid.get_checked_mut(coord) = None;
        }
    }
    fn shrink_candidate_by(&mut self, candidate: u32, by: u32) {
        for _ in 0..by {
            self.shrink_candidate(candidate);
        }
    }
    fn remove_sharp_edges(&mut self) {
        let mut to_remove = Vec::new();
        for (coord, cell) in self.grid.enumerate() {
            if cell.is_some() {
                if self
                    .grid
                    .get(coord + Coord::new(1, 0))
                    .cloned()
                    .flatten()
                    .is_none()
                    && self
                        .grid
                        .get(coord + Coord::new(-1, 0))
                        .cloned()
                        .flatten()
                        .is_none()
                {
                    to_remove.push(coord);
                } else if self
                    .grid
                    .get(coord + Coord::new(0, 1))
                    .cloned()
                    .flatten()
                    .is_none()
                    && self
                        .grid
                        .get(coord + Coord::new(0, -1))
                        .cloned()
                        .flatten()
                        .is_none()
                {
                    to_remove.push(coord);
                }
            }
        }
        for coord in to_remove {
            *self.grid.get_checked_mut(coord) = None;
        }
    }
    fn remove_small_pools(&mut self, min_size: usize) {
        let mut to_remove = Vec::new();
        let mut seen = HashSet::new();
        let mut flood_fill_buffer = VecDeque::new();
        for (coord, cell) in self.grid.enumerate() {
            if cell.is_some() {
                if seen.insert(coord) {
                    flood_fill_buffer.push_back(coord);
                    let mut current_region = Vec::new();
                    while let Some(coord) = flood_fill_buffer.pop_front() {
                        current_region.push(coord);
                        for direction in CardinalDirection::all() {
                            let neighbour_coord = coord + direction.coord();
                            if self.grid.get(neighbour_coord).cloned().flatten().is_some() {
                                if seen.insert(neighbour_coord) {
                                    flood_fill_buffer.push_back(neighbour_coord);
                                }
                            }
                        }
                    }
                    if current_region.len() < min_size {
                        to_remove.extend(current_region);
                    }
                }
            }
        }
        for coord in to_remove {
            *self.grid.get_checked_mut(coord) = None;
        }
    }
    fn add_pools(&self, grid: &Grid<CellA>) -> Grid<CellB> {
        Grid::new_grid_map_ref_with_coord(grid, |coord, cell| match cell {
            CellA::Closed => CellB::Wall,
            CellA::Open => {
                if self.grid.get_checked(coord).is_some() {
                    CellB::Pool
                } else {
                    CellB::Floor
                }
            }
        })
    }
}

fn add_outer_wall(grid: &Grid<CellB>) -> Grid<CellB> {
    let mut grid = grid.clone();
    for i in 0..grid.size().width() {
        let cell = grid.get_checked_mut(Coord::new(i as i32, 0));
        if let CellB::Floor = cell {
            *cell = CellB::Wall;
        }
        let cell = grid.get_checked_mut(Coord::new(i as i32, grid.size().height() as i32 - 1));
        if let CellB::Floor = cell {
            *cell = CellB::Wall;
        }
    }
    for i in 0..grid.size().height() {
        let cell = grid.get_checked_mut(Coord::new(0, i as i32));
        if let CellB::Floor = cell {
            *cell = CellB::Wall;
        }
        let cell = grid.get_checked_mut(Coord::new(grid.size().width() as i32 - 1, i as i32));
        if let CellB::Floor = cell {
            *cell = CellB::Wall;
        }
    }
    grid
}

fn remove_boring_space_step(grid: &mut Grid<CellB>) -> bool {
    let mut to_remove = Vec::new();
    for (coord, cell) in grid.enumerate() {
        if let CellB::Floor = cell {
            if let Some(CellB::Wall) = grid.get(coord + Coord::new(1, 0)) {
                if let Some(CellB::Wall) = grid.get(coord + Coord::new(-1, 0)) {
                    to_remove.push(coord);
                    continue;
                }
            }
        }
        if let CellB::Floor = cell {
            if let Some(CellB::Wall) = grid.get(coord + Coord::new(0, 1)) {
                if let Some(CellB::Wall) = grid.get(coord + Coord::new(0, -1)) {
                    to_remove.push(coord);
                    continue;
                }
            }
        }
    }
    let ret = !to_remove.is_empty();
    for coord in to_remove {
        *grid.get_checked_mut(coord) = CellB::Wall;
    }
    ret
}

fn remove_boring_space(grid: &Grid<CellB>) -> Grid<CellB> {
    let mut grid = grid.clone();
    while remove_boring_space_step(&mut grid) {}
    grid
}

fn classify_by_wall(grid: &Grid<CellB>) -> Grid<Option<usize>> {
    let mut by_room: Grid<Option<usize>> = Grid::new_grid_map_ref(grid, |_| None);
    let mut seen = HashSet::new();
    let mut flood_fill_buffer = VecDeque::new();
    let mut id = 0;
    for (coord, &cell) in grid.enumerate() {
        if cell != CellB::Wall {
            if seen.insert(coord) {
                flood_fill_buffer.push_back(coord);
                while let Some(coord) = flood_fill_buffer.pop_front() {
                    *by_room.get_checked_mut(coord) = Some(id);
                    for direction in CardinalDirection::all() {
                        let neighbour_coord = coord + direction.coord();
                        if let Some(&cell) = grid.get(neighbour_coord) {
                            if cell != CellB::Wall {
                                if seen.insert(neighbour_coord) {
                                    flood_fill_buffer.push_back(neighbour_coord);
                                }
                            }
                        }
                    }
                }
                id += 1;
            }
        }
    }
    by_room
}

fn classify_by_pool(grid: &Grid<CellB>) -> Grid<Option<usize>> {
    let mut by_pool: Grid<Option<usize>> = Grid::new_grid_map_ref(grid, |_| None);
    let mut seen = HashSet::new();
    let mut flood_fill_buffer = VecDeque::new();
    let mut id = 0;
    for (coord, &cell) in grid.enumerate() {
        if cell != CellB::Pool {
            if seen.insert(coord) {
                flood_fill_buffer.push_back(coord);
                while let Some(coord) = flood_fill_buffer.pop_front() {
                    *by_pool.get_checked_mut(coord) = Some(id);
                    for direction in CardinalDirection::all() {
                        let neighbour_coord = coord + direction.coord();
                        if let Some(&cell) = grid.get(neighbour_coord) {
                            if cell != CellB::Pool {
                                if seen.insert(neighbour_coord) {
                                    flood_fill_buffer.push_back(neighbour_coord);
                                }
                            }
                        }
                    }
                }
                id += 1;
            }
        }
    }
    by_pool
}

#[derive(Clone, Copy)]
struct ClassifiedFloor {
    by_wall: usize,
    by_pool: usize,
}

enum CellC {
    Wall,
    Pool,
    Floor(ClassifiedFloor),
}

fn classify_floor(grid: &Grid<CellB>) -> Grid<CellC> {
    let by_wall = classify_by_wall(grid);
    let by_pool = classify_by_pool(grid);
    Grid::new_fn(grid.size(), |coord| {
        if let Some(by_wall) = by_wall.get_checked(coord) {
            if let Some(by_pool) = by_pool.get_checked(coord) {
                CellC::Floor(ClassifiedFloor {
                    by_wall: *by_wall,
                    by_pool: *by_pool,
                })
            } else {
                CellC::Pool
            }
        } else {
            CellC::Wall
        }
    })
}

#[derive(Clone)]
struct BridgeCandidate {
    coords: Vec<Coord>,
    start: usize,
    end: usize,
}

struct BridgeCandidates {
    by_sides: HashMap<(usize, usize), Vec<BridgeCandidate>>,
}

fn bridge_candidates_axis(
    grid: &Grid<CellC>,
    bridge_aligned_to_axis: Axis,
) -> Vec<BridgeCandidate> {
    let mut candidates = Vec::new();
    for x in 0..(grid.size().get(bridge_aligned_to_axis.other()) as i32) {
        let mut by_pool_start = None;
        let mut pool_coords = Vec::new();
        for y in 0..(grid.size().get(bridge_aligned_to_axis) as i32) {
            let coord = Coord::new_axis(y, x, bridge_aligned_to_axis);
            match grid.get_checked(coord) {
                CellC::Floor(classified_floor) => {
                    if let Some(start) = by_pool_start {
                        if !pool_coords.is_empty() {
                            let end = classified_floor.by_pool;
                            if start != end {
                                candidates.push(BridgeCandidate {
                                    coords: pool_coords.clone(),
                                    start,
                                    end,
                                });
                            }
                        }
                    }
                    pool_coords.clear();
                    by_pool_start = Some(classified_floor.by_pool);
                }
                CellC::Wall => by_pool_start = None,
                CellC::Pool => pool_coords.push(coord),
            }
        }
    }
    candidates
}

impl BridgeCandidates {
    fn new(grid: &Grid<CellC>) -> BridgeCandidates {
        let mut candidates = bridge_candidates_axis(grid, Axis::X);
        candidates.append(&mut bridge_candidates_axis(grid, Axis::Y));
        let mut by_sides: HashMap<_, Vec<BridgeCandidate>> = HashMap::new();
        for candidate in candidates {
            let key = if candidate.start < candidate.end {
                (candidate.start, candidate.end)
            } else {
                (candidate.end, candidate.start)
            };
            by_sides.entry(key).or_default().push(candidate);
        }
        for candidates in by_sides.values_mut() {
            candidates.sort_by(|a, b| a.coords.len().cmp(&b.coords.len()));
            for _ in 0..(candidates.len() / 2) {
                candidates.pop();
            }
        }
        BridgeCandidates { by_sides }
    }
    fn choose<R: Rng>(&self, rng: &mut R) -> Vec<BridgeCandidate> {
        self.by_sides
            .values()
            .map(|candidates| candidates.choose(rng).unwrap().clone())
            .collect()
    }
}

#[derive(Clone)]
struct DoorCandidate {
    high: usize,
    low: usize,
    coords: Vec<Coord>,
}

impl DoorCandidate {
    fn choose<R: Rng>(&self, rng: &mut R) -> Coord {
        let low_index = self.coords.len() / 4;
        let high_index = (self.coords.len() - 1 - low_index).max(low_index + 1);
        self.coords[rng.gen_range(low_index..high_index)]
    }
}

fn door_candidates_axis(grid: &Grid<CellC>, wall_aligned_to_axis: Axis) -> Vec<DoorCandidate> {
    let mut candidates = Vec::new();
    for x in 1..(grid.size().get(wall_aligned_to_axis.other()) as i32 - 1) {
        let mut in_progress = false;
        for y in 1..(grid.size().get(wall_aligned_to_axis) as i32 - 1) {
            let coord = Coord::new_axis(y, x, wall_aligned_to_axis);
            if let CellC::Wall = grid.get_checked(coord) {
                let high_coord = coord + Coord::new_axis(0, 1, wall_aligned_to_axis);
                let low_coord = coord - Coord::new_axis(0, 1, wall_aligned_to_axis);
                if let CellC::Floor(high) = grid.get_checked(high_coord) {
                    if let CellC::Floor(low) = grid.get_checked(low_coord) {
                        if high.by_wall != low.by_wall {
                            if !in_progress {
                                in_progress = true;
                                candidates.push(DoorCandidate {
                                    high: high.by_wall,
                                    low: low.by_wall,
                                    coords: Vec::new(),
                                });
                            }
                            let current = candidates.last_mut().unwrap();
                            assert_eq!(current.high, high.by_wall);
                            assert_eq!(current.low, low.by_wall);
                            current.coords.push(coord);
                            continue;
                        }
                    }
                }
                in_progress = false;
            }
        }
    }
    candidates
}

struct DoorCandidates {
    candidates: Vec<DoorCandidate>,
}

impl DoorCandidates {
    fn new(grid: &Grid<CellC>) -> Self {
        let mut candidates = door_candidates_axis(grid, Axis::X);
        candidates.append(&mut door_candidates_axis(grid, Axis::Y));
        Self { candidates }
    }
    fn graph(&self) -> DoorCandidateGraph {
        let mut graph: DoorCandidateGraph = HashMap::new();
        for (door_candidate_index, door_candidate) in self.candidates.iter().enumerate() {
            graph
                .entry(door_candidate.low)
                .or_default()
                .edges
                .push(RoomEdge {
                    to_room: door_candidate.high,
                    via_door_candidate: door_candidate_index,
                });
            graph
                .entry(door_candidate.high)
                .or_default()
                .edges
                .push(RoomEdge {
                    to_room: door_candidate.low,
                    via_door_candidate: door_candidate_index,
                });
        }
        graph
    }
    fn minimum_spanning_tree<R: Rng>(&self, rng: &mut R) -> HashSet<DoorCandidateIndex> {
        let door_candidate_graph = self.graph();
        let mut mst = HashSet::new();
        let mut visited_room_ids = HashSet::new();
        if self.candidates.is_empty() {
            return mst;
        }
        let mut to_visit = vec![rng.gen_range(0..self.candidates.len())];
        while !to_visit.is_empty() {
            let door_candidate_id = to_visit.swap_remove(rng.gen_range(0..to_visit.len()));
            let door_candidate = &self.candidates[door_candidate_id];
            let new_low = visited_room_ids.insert(door_candidate.low);
            let new_high = visited_room_ids.insert(door_candidate.high);
            if !(new_low || new_high) {
                continue;
            }
            mst.insert(door_candidate_id);
            for edge in door_candidate_graph[&door_candidate.low]
                .edges
                .iter()
                .chain(door_candidate_graph[&door_candidate.high].edges.iter())
            {
                if !visited_room_ids.contains(&edge.to_room) {
                    to_visit.push(edge.via_door_candidate);
                }
            }
        }
        mst
    }
    fn choose<R: Rng>(&self, rng: &mut R) -> Vec<DoorCandidate> {
        let mst = self.minimum_spanning_tree(rng);
        let mut chosen_door_candidate_indices = mst.iter().cloned().collect::<Vec<_>>();
        let mut other_door_indices = (0..self.candidates.len())
            .filter(|i| !mst.contains(i))
            .collect::<Vec<_>>();
        other_door_indices.shuffle(rng);
        let num_other_door_candidates = other_door_indices.len() / 4;
        chosen_door_candidate_indices.extend(
            other_door_indices
                .into_iter()
                .take(num_other_door_candidates),
        );
        chosen_door_candidate_indices.sort();
        chosen_door_candidate_indices
            .into_iter()
            .map(|i| self.candidates[i].clone())
            .collect()
    }
}

type DoorCandidateIndex = usize;

struct RoomEdge {
    to_room: usize,
    via_door_candidate: DoorCandidateIndex,
}

#[derive(Default)]
struct RoomNode {
    edges: Vec<RoomEdge>,
}

type DoorCandidateGraph = HashMap<usize, RoomNode>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SewerCell {
    Floor,
    Wall,
    Pool,
    Bridge,
    Door,
}

fn make_cell_d_grid(grid: &Grid<CellC>) -> Grid<SewerCell> {
    Grid::new_grid_map_ref(grid, |cell| match cell {
        CellC::Floor(_) => SewerCell::Floor,
        CellC::Pool => SewerCell::Pool,
        CellC::Wall => SewerCell::Wall,
    })
}

fn add_bridge_candidate(grid: &mut Grid<SewerCell>, candidate: &BridgeCandidate) {
    for &coord in candidate.coords.iter() {
        *grid.get_checked_mut(coord) = SewerCell::Bridge;
    }
}

fn ensure_single_connected_area(grid: &mut Grid<SewerCell>) {
    let mut areas = Vec::new();
    let mut seen = HashSet::new();
    let mut flood_fill_buffer = VecDeque::new();
    for (coord, &cell) in grid.enumerate() {
        if cell != SewerCell::Wall {
            if seen.insert(coord) {
                flood_fill_buffer.push_back(coord);
                let mut area = Vec::new();
                while let Some(coord) = flood_fill_buffer.pop_front() {
                    area.push(coord);
                    for direction in CardinalDirection::all() {
                        let neighbour_coord = coord + direction.coord();
                        if let Some(&cell) = grid.get(neighbour_coord) {
                            if cell != SewerCell::Wall {
                                if seen.insert(neighbour_coord) {
                                    flood_fill_buffer.push_back(neighbour_coord);
                                }
                            }
                        }
                    }
                }
                areas.push(area);
            }
        }
    }
    let index_of_largest_area = areas
        .iter()
        .map(|a| a.len())
        .enumerate()
        .max_by_key(|&(_index, len)| len)
        .unwrap()
        .0;
    for (index, area) in areas.iter_mut().enumerate() {
        if index != index_of_largest_area {
            for &coord in area.iter() {
                *grid.get_checked_mut(coord) = SewerCell::Wall;
            }
        }
    }
}

fn all_floor_adjacent_floor_coords(grid: &Grid<SewerCell>) -> Vec<Coord> {
    grid.enumerate()
        .filter_map(|(coord, &cell)| {
            if cell == SewerCell::Floor {
                if Direction::all()
                    .map(|d| grid.get(coord + d.coord()).cloned())
                    .all(|maybe_cell| maybe_cell == Some(SewerCell::Floor))
                {
                    Some(coord)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

fn pool_light_coords<R: Rng>(grid: &Grid<SewerCell>, rng: &mut R) -> Vec<Coord> {
    let mut coords = Vec::new();
    for (coord, cell) in grid.enumerate() {
        if let SewerCell::Pool = cell {
            if rng.gen_range(0..20) == 0
                || CardinalDirection::all()
                    .map(|d| grid.get(coord + d.coord()).cloned())
                    .any(|maybe_cell| maybe_cell == Some(SewerCell::Floor) || maybe_cell.is_none())
            {
                coords.push(coord)
            }
        }
    }
    coords
}

#[derive(Clone, Copy)]
pub struct SewerSpec {
    pub size: Size,
}

pub enum SewerLightType {
    Pool,
}

pub struct SewerLight {
    pub typ: SewerLightType,
    pub coord: Coord,
}

pub struct Sewer {
    pub start: Coord,
    pub goal: Coord,
    pub map: Grid<SewerCell>,
    pub lights: Vec<SewerLight>,
}

impl Sewer {
    pub fn generate<R: Rng>(spec: SewerSpec, rng: &mut R) -> Self {
        loop {
            if let Some(sewer) = Self::try_generate(spec, rng) {
                return sewer;
            }
        }
    }
    pub fn try_generate<R: Rng>(spec: SewerSpec, rng: &mut R) -> Option<Self> {
        let pattern_size = NonZeroU32::new(3).unwrap();
        let map = wfc_map(
            input_grid_from_strs(WFC_INPUT),
            spec.size,
            pattern_size,
            rng,
        );
        let mut pool_candidates = PoolCandidates::new(&map);
        for candidate in 0..pool_candidates.num {
            let shrink_by = rng.gen_range(2..4);
            pool_candidates.shrink_candidate_by(candidate, shrink_by);
        }
        pool_candidates.remove_sharp_edges();
        pool_candidates.remove_sharp_edges();
        pool_candidates.remove_sharp_edges();
        pool_candidates.remove_small_pools(8);
        let map = pool_candidates.add_pools(&map);
        let map = add_outer_wall(&map);
        let map = remove_boring_space(&map);
        let classified_map = classify_floor(&map);
        let bridge_candidates = BridgeCandidates::new(&classified_map);
        let door_candidates = DoorCandidates::new(&classified_map);
        let mut map = make_cell_d_grid(&classified_map);
        for candidate in bridge_candidates.choose(rng) {
            add_bridge_candidate(&mut map, &candidate);
        }
        let door_coords = door_candidates
            .choose(rng)
            .into_iter()
            .map(|candidate| candidate.choose(rng))
            .collect::<Vec<_>>();
        for coord in door_coords {
            *map.get_checked_mut(coord) = SewerCell::Door;
        }
        ensure_single_connected_area(&mut map);
        let mut player_and_goal_candidates = all_floor_adjacent_floor_coords(&map);
        player_and_goal_candidates.shuffle(rng);
        let start = player_and_goal_candidates.pop()?;
        player_and_goal_candidates.sort_by_key(|coord| coord.distance2(start));
        let goal_start_offset = 9 * (player_and_goal_candidates.len() / 10);
        let goal = player_and_goal_candidates[goal_start_offset..]
            .choose(rng)?
            .clone();
        if !map.iter().any(|&cell| cell == SewerCell::Pool) {
            return None;
        }
        let lights = pool_light_coords(&map, rng)
            .into_iter()
            .map(|coord| SewerLight {
                coord,
                typ: SewerLightType::Pool,
            })
            .collect::<Vec<_>>();
        let sewer = Sewer {
            start,
            goal,
            map,
            lights,
        };
        Some(sewer)
    }
}
