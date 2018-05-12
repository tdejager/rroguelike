extern crate tcod;
extern crate rand;

use tcod::console::*;
use tcod::colors;
use tcod::Color;
use tcod::map::{FovAlgorithm, Map as FovMap};
use rand::Rng;
use std::cmp;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 20;

// Map properties
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;

// Wall properties
const COLOR_DARK_WALL: Color = Color {r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color {r: 130, g: 110, b: 50};
const COLOR_DARK_GROUND: Color = Color {r: 50, g: 50, b: 150 };
const COLOR_LIGHT_GROUND: Color = Color {r: 200, g: 180, b: 50};

// Room properties
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 10;
const MAX_ROOMS: i32 = 10;

// Fov algo
const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

// Monster stuff
const MAX_ROOM_MONSTERS: i32 = 3;

const PLAYER: usize = 0;



#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {x1: x, y1: y, x2: x + w, y2: y + h}
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)

    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        // returns true if this rectangle intersects wit another one
        (self.x1 <= other.x2) && (self.x2 >= other.x1) &&
            (self.y1 <= other.y2) && (self.y2 >= other.y1)
    }

}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    let y_min = cmp::min(y1, y2);
    let y_max = cmp::max(y1, y2);
    for y in y_min..y_max + 1 {
        map[x as usize][y as usize] = Tile::empty();
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Tile {
    blocked: bool,
    block_sight: bool,
    explored: bool,
}
type Map = Vec<Vec<Tile>>;

impl Tile {
    pub fn empty() -> Self {
        Tile{blocked: false, explored: false, block_sight: false}
    }

    pub fn wall() -> Self {
        Tile{blocked: true, explored: false, block_sight: true}
    }
}

fn make_map(objects: &mut Vec<Object>) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    let mut rooms : Vec<Rect> = vec![];
    let mut starting_position = (0, 0);
    let mut player = Object::new(starting_position.0, starting_position.1, '@', "player", colors::WHITE, true);
    player.alive = true;

    objects.push(player);
    for room_idx in 0..MAX_ROOMS {

        println!("Creating room {}", room_idx);
        // random width and height
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        // random position without going out of the boundaries of the map
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);

        let failed = rooms.iter().any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            create_room(new_room, &mut map);
            // Add some content to the this room, such as monsters
            place_objects(new_room, &mut map, objects);

            let (new_x, new_y) = new_room.center();

            if rooms.is_empty() {
                starting_position = (new_x, new_y);
            } else {
                // all rooms after the first:
                // connect it to the previous room with a tunnel

                // center coordinates of the previous room
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                // toss a coin (random bool value -- either true or false)
                if rand::random() {
                    // first move horizontally, then vertically
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    // first move vertically, then horizontally
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            }
            rooms.push(new_room);
        }
    }
    objects[PLAYER].set_pos(starting_position.0, starting_position.1);
    map
}

/// An object in the game
#[derive(Debug)]
pub struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool
}

/// Implementation of the object
impl Object {
    pub fn new(x: i32, y: i32, char: char, name: &str, color: Color, blocks: bool) -> Self {
        Object {
            x: x,
            y: y,
            char: char,
            color: color,
            name: name.into(),
            blocks: blocks,
            alive: false,
        }
    }


    /// Set the color and then draw the character that represents this object at its position
    pub fn draw(&self, con: &mut Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    /// Erase the character that represents this object
    pub fn clear(&self, con: &mut Console) {
        con.put_char(self.x, self.y, ' ', BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }
}


pub fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

pub fn player_move_or_attack(dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    let target_id = objects.iter().position(|object| {
        object.pos() == (x, y)
    });

    match target_id {
        // A monster was found
        Some(id) => {
            println!("The {} laughs at your puny effort to attack him!", objects[id].name);
        }
        // No monster was found
        None => move_by(PLAYER, dx, dy, map, objects)
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    // choose random number of monsters
    //
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters {
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        if !is_blocked(x, y, map, objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                Object::new(x, y, 'o', "orc", colors::GREEN, true)
            } else {
                Object::new(x, y, 'T', "Troll", colors::DARKER_GREEN, true)
            };
            monster.alive = true;
            objects.push(monster);
        }
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    // first test the map tile
    if map[x as usize][y as usize].blocked {
        return true;
    }
    //println!("x {}, y {} is blocked", x, y);

    // now check for any blocking objects
   let monster_block = objects.iter().any(|object| {
        object.blocks && object.pos() == (x, y)
    });

    //println!("blocked by object");
    monster_block

}

fn handle_keys(root: &mut Root, objects: &mut Vec<Object>, map: &Map) -> PlayerAction  {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;
    use PlayerAction::*;

    let player_alive = objects[PLAYER].alive;
    let key: Key = root.wait_for_keypress(true);

    match (key, player_alive) {
        // Toggle fullscreen
        (Key { code: Enter, ctrl: true, ..} , true)=> {
            let fullscreen = root.is_fullscreen();
            root.set_fullscreen(fullscreen);
            DidntTakeTurn
        }
        // movement keys
        (Key { code: Up, .. }, true)    => {
            player_move_or_attack(0, -1, map, objects);
            TookTurn
        },
        (Key { code: Down, .. }, true)  => {
            player_move_or_attack(0, 1, map, objects);
            TookTurn
        },
        (Key { code: Left, .. }, true)  => {
            player_move_or_attack(-1, 0, map, objects);
            TookTurn
        },
        (Key { code: Right, .. }, true) => {
            player_move_or_attack(1, 0, map, objects);
            TookTurn
        },
        (Key { code: Escape, ..}, _) => { Exit }

        _ => DidntTakeTurn,
    }
}

fn render_all(root: &mut Root, con: &mut Offscreen, objects: &[Object], map: &mut Map, fov_map: &mut tcod::map::Map, fov_recompute: bool) {

    if fov_recompute {
        // Recompute fov if needed
        let player :&Object = &objects[PLAYER];

        fov_map.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);

        // go through all tiles, and set their background color
            for y in 0..MAP_HEIGHT {
                for x in 0..MAP_WIDTH {
                    let visible = fov_map.is_in_fov(x, y);
                    let wall = map[x as usize][y as usize].block_sight;
                    let color = match (visible, wall) {
                        // outside of field of view:
                        (false, true) => COLOR_DARK_WALL,
                        (false, false) => COLOR_DARK_GROUND,
                        // inside fov:
                        (true, true) => COLOR_LIGHT_WALL,
                        (true, false) => COLOR_LIGHT_GROUND,
                    };

                    let explored = &mut map[x as usize][y as usize].explored;
                    if visible {
                        *explored = true;
                    }

                    if *explored {
                        con.set_char_background(x, y, color, BackgroundFlag::Set);
                    }
                }
            }
    }
    for object in objects {
        object.draw(con);
    }


    blit(con, (0, 0), (SCREEN_WIDTH,SCREEN_HEIGHT)
         ,root, (0, 0), 1.0, 1.0);
}


fn main() {

    let mut root = Root::initializer()
        .font("/Users/timdejager/.cargo/registry/src/github.com-1ecc6299db9ec823/tcod-0.12.1/fonts/consolas12x12_gs_tc.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();

    let mut con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);

    tcod::system::set_fps(LIMIT_FPS);

    let mut objects : Vec<Object> = vec![];
    let mut map = make_map(&mut objects);

    let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH{
            fov_map.set(x, y,
                        !map[x as usize][y as usize].block_sight,
                        !map[x as usize][y as usize].blocked);
        }
    }

    let mut previous_player_position = (-1, -1);
    while !root.window_closed() {
        con.set_default_foreground(colors::WHITE);

        let fov_recompute = previous_player_position != objects[PLAYER].pos();
        render_all(&mut root, &mut con, &objects, &mut map, &mut fov_map, fov_recompute);
        root.flush();

        for object in &objects {
            object.clear(&mut con);
        }

        // Check for exit and handle keys
        {
            let player = &objects[PLAYER];
            previous_player_position = (player.x, player.y);
        }
        let player_action = handle_keys(&mut root, &mut objects, &map);

        if player_action == PlayerAction::Exit{
            break
        }

        if objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            for object in &objects {

                if (object as *const _) != (&objects[PLAYER] as *const _) {
                    println!("The {} growls", object.name);
                }
            }
        }
    }
}


