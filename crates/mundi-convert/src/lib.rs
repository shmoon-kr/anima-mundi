//! tbaMUD world files to the content format (D15). The binary is `mundi-convert`.

pub mod tba;

use std::path::Path;

use mundi_content::ZoneContent;

/// Convert one zone (its .wld .mob .obj .shp and the zone's resets).
pub fn convert_zone(world: &Path, z: &tba::ZoneHead, ids: &tba::Ids, notes: &mut tba::Notes) -> ZoneContent {
    let file = |dir: &str, ext: &str| world.join(dir).join(format!("{}.{ext}", z.num));
    let mut rooms = tba::read_rooms(&file("wld", "wld"), ids, notes);
    let resets = tba::resets(z, ids, &mut rooms, notes);
    ZoneContent {
        zone: Some(tba::zone(z, notes)),
        rooms,
        mobs: tba::read_mobs(&file("mob", "mob"), ids, notes),
        objects: tba::read_objects(&file("obj", "obj"), ids, notes),
        resets,
        shops: tba::read_shops(&file("shp", "shp"), ids, notes),
    }
}
