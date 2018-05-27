extern crate tcod;
extern crate rand;

use tcod::console::*;
use tcod::colors;
use tcod::Color;
use tcod::map::{FovAlgorithm, Map as FovMap};
use rand::Rng;
use std::cmp;

// Screen globals
const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 20;
// Sizes and coordinates relevant for the GUI
const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;
// Message log constants
const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;
type Messages = Vec<(String, Color)>;

// Map properties
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 43;

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

// The player index
const PLAYER: usize = 0;

#[derive(Copy, Clone, Debug, PartialEq)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object) {
        use DeathCallback::*;
        let callback: fn(&mut Object) = match self {
                Player => player_death,
                Monster => monster_death,
            };
        callback(object);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
    on_death: DeathCallback,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ai;


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
    player.fighter = Some(Fighter{max_hp: 30, hp: 30, defense: 2, power: 5, on_death: DeathCallback::Player});

    objects.push(player);
    for room_idx in 0..MAX_ROOMS {

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
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<Ai>
}

/// Implementation of the object
impl Object {
    pub fn new(x: i32, y: i32, char: char, name: &str, color: Color, blocks: bool) -> Self {
        Object {
            x,
            y,
            char,
            color,
            name: name.into(),
            blocks,
            alive: false,
            fighter: None,
            ai: None,
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

    pub fn take_damage(&mut self, damage: i32) {
        // apply damage if possible
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }

        // check for death, call the death function
        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self);
            }
        }
    }

    pub fn attack(&mut self, target: &mut Object, messages: &mut Messages) {
        // a simple formula for attack damage
        let damage = self.fighter.map_or(0, |f| f.power) - target.fighter.map_or(0, |f| f.defense);
        if damage > 0 {
            // Make the target take some damage
            message(messages, format!("{} attacks {} for {} hit points", self.name, target.name, damage), self.color);
            target.take_damage(damage);
        } else {
            message(messages, format!("{} attack {} but it has no effect!", self.name, target.name), colors::DARK_YELLOW);
        }
    }

    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }
}

fn player_death(player: &mut Object) {
    // The game ended
    println!("You died!");
    player.char = '%';
    player.color = colors::DARK_RED;
}

fn monster_death(monster: &mut Object) {
    // Transform it into a nasty corpse, it doesn't block, can't be attacked
    // and doesn't move
    println!("{} is dead!", monster.name);
    monster.char = '%';
    monster.color = colors::DARK_RED;
    monster.blocks = false;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
}

pub fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

pub fn player_move_or_attack(dx: i32, dy: i32, map: &Map, objects: &mut [Object], messages: &mut Messages) {
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    let target_id = objects.iter().position(|object| {
        object.fighter.is_some() && object.pos() == (x, y)
    });

    match target_id {
        // A monster was found
        Some(id) => {
            let (player, target) = mut_two(PLAYER, id, objects);
            player.attack(target, messages);
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
                let mut orc = Object::new(x, y, 'o', "orc", colors::GREEN, true);
                orc.fighter = Some(Fighter{max_hp: 10, hp: 10, defense: 0, power: 3, on_death: DeathCallback::Monster});
                orc.ai = Some(Ai);
                orc
            } else {
                let mut troll = Object::new(x, y, 'T', "Troll", colors::DARKER_GREEN, true);
                troll.fighter = Some(Fighter{max_hp: 16, hp: 16, defense: 1, power: 4, on_death: DeathCallback::Monster});
                troll.ai = Some(Ai);
                troll
            };
            monster.alive = true;
            objects.push(monster);
        }
    }
}

fn mut_two<T>(first_index: usize, second_index: usize, items: &mut[T]) -> (&mut T, &mut T) {
    assert_ne!(first_index, second_index);
    let split_at_index = cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);

    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }

}

fn ai_take_turn(monster_id: usize, map: &Map, objects: &mut [Object], fov_map: &FovMap, messages: &mut Messages) {
    // a basic monster takes its turn. If you can see it, it can see you
    let (monster_x, monster_y) = objects[monster_id].pos();
    // TODO finish AI take turn
    if fov_map.is_in_fov(monster_x, monster_y) {
        if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
            // move towards player if far away
            let (player_x, player_y) = objects[PLAYER].pos();
            move_towards(monster_id, player_x, player_y, map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
            // Close enough to attack if the player is still alive
            let (monster, player) = mut_two(monster_id, PLAYER, objects);

            monster.attack(player, messages);
        }
    }
}

fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut[Object]) {
    // vector from this object to the target, and distance
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

    // Normalize it to length 1 then round and convert to integer
    // so that the movement is restricted to a grid
    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
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

fn handle_keys(root: &mut Root, objects: &mut Vec<Object>, map: &Map, messages: &mut Messages) -> PlayerAction  {
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
            player_move_or_attack(0, -1, map, objects, messages);
            TookTurn
        },
        (Key { code: Down, .. }, true)  => {
            player_move_or_attack(0, 1, map, objects, messages);
            TookTurn
        },
        (Key { code: Left, .. }, true)  => {
            player_move_or_attack(-1, 0, map, objects, messages);
            TookTurn
        },
        (Key { code: Right, .. }, true) => {
            player_move_or_attack(1, 0, map, objects, messages);
            TookTurn
        },
        (Key { code: Escape, ..}, _) => { Exit }

        _ => DidntTakeTurn,
    }
}

fn render_all(root: &mut Root,
              con: &mut Offscreen,
              objects: &[Object],
              map: &mut Map,
              fov_map: &mut tcod::map::Map,
              fov_recompute: bool,
              panel: &mut Offscreen, messages: &Messages) {

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
    let mut to_draw: Vec<_> =
        objects.iter().filter(|o| fov_map.is_in_fov(o.x, o.y)).collect();
    // Sort so that non-blocking objects come first
    to_draw.sort_by(|o1, o2| {o1.blocks.cmp(&o2.blocks)});
    // Draw the objects in the list
    for object in &to_draw {
        object.draw(con);
    }

    blit(con, (0, 0), (SCREEN_WIDTH,SCREEN_HEIGHT), root, (0, 0), 1.0, 1.0);

    // Render the GUI
    // prepare to render the GUI panel
    panel.set_default_background(colors::BLACK);
    panel.clear();

    // Show the player stats
    let hp = objects[PLAYER].fighter.map_or(0, |f| f.hp);
    let max_hp = objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
    render_bar(panel, 1, 1, BAR_WIDTH,
               "HP",
               hp,
               max_hp,
               colors::LIGHT_RED, colors::DARKER_RED);

    // print the game messages one line at a time
    let mut y = MSG_HEIGHT as i32;
    for &(ref msg, color) in messages.iter().rev() {
        let msg_height = panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        y -= msg_height;
        if y < 0 {
            break;
        }
        panel.set_default_foreground(color);
        panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
    }

    // blit the contents of `panel` to the root console
    blit(panel, (0, 0),
         (SCREEN_WIDTH, PANEL_HEIGHT),
         root, (0, PANEL_Y),
         1.0, 1.0)
}

fn message<T: Into<String>>(messages: &mut Messages, message: T, color: Color) {
    // If the buffer is full, remove the first message to make room for the new one
    if messages.len() == MSG_HEIGHT {
        messages.remove(0);
    }

    messages.push((message.into(), color));
}

fn render_bar(panel: &mut Offscreen,
              x: i32,
              y: i32,
              total_width: i32,
              name: &str,
              value: i32,
              maximum: i32,
              bar_color: Color,
              back_color: Color) {

    // render a bar (HP, experience, etc). First calculate the width of the bar
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // render the background first
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // now render the bar on top
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    // finally, some centered text with values
    panel.set_default_foreground(colors::WHITE);
    panel.print_ex(x + total_width / 2, y, BackgroundFlag::None,
                   TextAlignment::Center, &format!("{}: {}/{}", name, value, maximum));
}


fn main() {

    let mut root = Root::initializer()
        .font("/Users/timdejager/.cargo/registry/src/github.com-1ecc6299db9ec823/tcod-0.12.1/fonts/consolas12x12_gs_tc.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();

    let mut con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);
    let mut panel = Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT);

    tcod::system::set_fps(LIMIT_FPS);

    let mut objects : Vec<Object> = vec![];
    let mut map = make_map(&mut objects);
    // Create the list of game messages and their color. starts empty
    let mut messages = vec![];

    let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH{
            fov_map.set(x, y,
                        !map[x as usize][y as usize].block_sight,
                        !map[x as usize][y as usize].blocked);
        }
    }

    // Print welcome message
    message(&mut messages,
            "Welcome stranger! Prepare to perish in the ST horror dungeon",
            colors::RED);

    let mut previous_player_position = (-1, -1);
    while !root.window_closed() {
        con.set_default_foreground(colors::WHITE);

        let fov_recompute = previous_player_position != objects[PLAYER].pos();
        render_all(&mut root, &mut con,
                   &objects, &mut map,
                   &mut fov_map,
                   fov_recompute,
                   &mut panel, &messages);
        root.flush();

        for object in &objects {
            object.clear(&mut con);
        }

        // Check for exit and handle keys
        {
            let player = &objects[PLAYER];
            previous_player_position = (player.x, player.y);
        }
        let player_action = handle_keys(&mut root, &mut objects, &map, &mut messages);

        if player_action == PlayerAction::Exit{
            break
        }

        if objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, &map, &mut objects, &fov_map, &mut messages);
                }
            }
        }
    }
}


