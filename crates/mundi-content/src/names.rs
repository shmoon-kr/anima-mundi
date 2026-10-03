//! Named values of the content format. Each is an enum, so a misspelt flag or sector is a load
//! error, not a silent default. The names and their order follow the tbaMUD tables the converter
//! reads (constants.c, structs.h); `ALL` is that order.

use serde::{Deserialize, Serialize};

macro_rules! named {
    ($(#[$m:meta])* $name:ident { $($var:ident = $s:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name { $( #[serde(rename = $s)] $var ),* }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),*];
            pub fn name(self) -> &'static str { match self { $($name::$var => $s),* } }
            pub fn from_index(i: usize) -> Option<$name> { Self::ALL.get(i).copied() }
        }
    };
}

named!(
    /// Exit directions, in the order rooms list them.
    Dir { North = "north", East = "east", South = "south", West = "west", Up = "up", Down = "down" }
);
named!(Sector {
    Inside = "inside", City = "city", Field = "field", Forest = "forest", Hills = "hills",
    Mountains = "mountains", WaterSwim = "water_swim", WaterNoswim = "water_noswim", Flying = "flying",
    Underwater = "underwater",
});
named!(RoomFlag {
    Dark = "dark", Death = "death", NoMob = "no_mob", Indoors = "indoors", Peaceful = "peaceful",
    Soundproof = "soundproof", NoTrack = "no_track", NoMagic = "no_magic", Tunnel = "tunnel", Private = "private",
    Godroom = "godroom", House = "house", HouseCrash = "house_crash", Atrium = "atrium", Olc = "olc",
    BfsMark = "bfs_mark", Worldmap = "worldmap",
});
named!(ZoneFlag {
    Closed = "closed", NoImmortal = "no_immortal", Quest = "quest", Grid = "grid", NoBuild = "no_build",
    NoAstral = "no_astral", Worldmap = "worldmap",
});
named!(ResetWhen { Never = "never", WhenEmpty = "when_empty", Always = "always" });
named!(DoorKind { Door = "door", Pickproof = "pickproof" });
named!(DoorState { Open = "open", Closed = "closed", Locked = "locked" });
named!(
    /// Mob action flags. tbaMUD's "is an NPC" bit is implied by being in mobs.yaml and is not listed.
    MobFlag {
        Spec = "spec", Sentinel = "sentinel", Scavenger = "scavenger", IsNpc = "isnpc", Aware = "aware",
        Aggressive = "aggressive", StayZone = "stay_zone", Wimpy = "wimpy", AggrEvil = "aggr_evil",
        AggrGood = "aggr_good", AggrNeutral = "aggr_neutral", Memory = "memory", Helper = "helper",
        NoCharm = "no_charm", NoSummon = "no_summon", NoSleep = "no_sleep", NoBash = "no_bash", NoBlind = "no_blind",
        NoKill = "no_kill", Dead = "dead",
    }
);
named!(
    /// Affects; tbaMUD's bit 0 is unused, so bit n is `ALL[n - 1]`.
    Affect {
        Blind = "blind", Invisible = "invisible", DetectAlign = "detect_align", DetectInvis = "detect_invis",
        DetectMagic = "detect_magic", SenseLife = "sense_life", Waterwalk = "waterwalk", Sanctuary = "sanctuary",
        Group = "group", Curse = "curse", Infravision = "infravision", Poison = "poison",
        ProtectEvil = "protect_evil", ProtectGood = "protect_good", Sleep = "sleep", NoTrack = "no_track",
        Fly = "fly", Scuba = "scuba", Sneak = "sneak", Hide = "hide", Unused = "unused", Charm = "charm",
    }
);
named!(Position {
    Dead = "dead", MortallyWounded = "mortally_wounded", Incapacitated = "incapacitated", Stunned = "stunned",
    Sleeping = "sleeping", Resting = "resting", Sitting = "sitting", Fighting = "fighting", Standing = "standing",
});
named!(Sex { Neutral = "neutral", Male = "male", Female = "female" });
named!(ItemType {
    Undefined = "undefined", Light = "light", Scroll = "scroll", Wand = "wand", Staff = "staff", Weapon = "weapon",
    Furniture = "furniture", Free = "free", Treasure = "treasure", Armor = "armor", Potion = "potion", Worn = "worn",
    Other = "other", Trash = "trash", Free2 = "free2", Container = "container", Note = "note", Drinkcon = "drinkcon",
    Key = "key", Food = "food", Money = "money", Pen = "pen", Boat = "boat", Fountain = "fountain",
});
named!(Wear {
    Take = "take", Finger = "finger", Neck = "neck", Body = "body", Head = "head", Legs = "legs", Feet = "feet",
    Hands = "hands", Arms = "arms", Shield = "shield", About = "about", Waist = "waist", Wrist = "wrist",
    Wield = "wield", Hold = "hold",
});
named!(ObjFlag {
    Glow = "glow", Hum = "hum", NoRent = "no_rent", NoDonate = "no_donate", NoInvis = "no_invis",
    Invisible = "invisible", Magic = "magic", NoDrop = "no_drop", Bless = "bless", AntiGood = "anti_good",
    AntiEvil = "anti_evil", AntiNeutral = "anti_neutral", AntiMage = "anti_mage", AntiCleric = "anti_cleric",
    AntiThief = "anti_thief", AntiWarrior = "anti_warrior", NoSell = "no_sell", QuestItem = "quest_item",
});
named!(Attack {
    Hit = "hit", Sting = "sting", Whip = "whip", Slash = "slash", Bite = "bite", Bludgeon = "bludgeon",
    Crush = "crush", Pound = "pound", Claw = "claw", Maul = "maul", Thrash = "thrash", Pierce = "pierce",
    Blast = "blast", Punch = "punch", Stab = "stab",
});
named!(Liquid {
    Water = "water", Beer = "beer", Wine = "wine", Ale = "ale", DarkAle = "dark_ale", Whisky = "whisky",
    Lemonade = "lemonade", Firebreather = "firebreather", LocalSpeciality = "local_speciality",
    SlimeMoldJuice = "slime_mold_juice", Milk = "milk", Tea = "tea", Coffee = "coffee", Blood = "blood",
    SaltWater = "salt_water", ClearWater = "clear_water",
});
named!(
    /// Where a zone reset puts equipment on a mob (tbaMUD wear positions).
    EquipPos {
        Light = "light", FingerRight = "finger_right", FingerLeft = "finger_left", Neck1 = "neck_1", Neck2 = "neck_2",
        Body = "body", Head = "head", Legs = "legs", Feet = "feet", Hands = "hands", Arms = "arms", Shield = "shield",
        About = "about", Waist = "waist", WristRight = "wrist_right", WristLeft = "wrist_left", Wield = "wield",
        Hold = "hold",
    }
);
named!(
    /// What an object changes while worn. tbaMUD numbers these with gaps; `Apply::from_code` maps them.
    Apply {
        Str = "str", Dex = "dex", Int = "int", Wis = "wis", Con = "con", Cha = "cha", Class = "class", Level = "level",
        Age = "age", Weight = "weight", Height = "height", Mana = "mana", Hit = "hit", Move = "move", Gold = "gold",
        Exp = "exp", Ac = "ac", Hitroll = "hitroll", Damroll = "damroll", SavePara = "save_para", SaveRod = "save_rod",
        SavePetri = "save_petri", SaveBreath = "save_breath", SaveSpell = "save_spell",
    }
);

impl Apply {
    /// tbaMUD apply codes start at 1 (0 is "none").
    pub fn from_code(code: i64) -> Option<Apply> {
        usize::try_from(code).ok().filter(|c| *c >= 1).and_then(|c| Apply::from_index(c - 1))
    }
}
