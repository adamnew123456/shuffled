use random;
use std::convert::TryInto;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;
use std::str;
use std::time::SystemTime;

/// Describes where the separates stages of the reader process should write
/// their data to
#[derive(Debug, PartialEq)]
pub struct FileOutputs<'a> {
    pub mono_wav: &'a Path,
    pub stereo_wav: &'a Path,
    pub lame_mp3: &'a Path,
    pub final_mp3: &'a Path,
}

/// A list of all the common ID3 genres. Everything starting from Blues to
/// HardRock is part of the ID3v1 specification while everything after HardRock
/// is recognized by various versions of WinAmp. See the Mutagen documentation
/// for a full list. Note that Unknown isn't included as it's a catch-all for
/// all the unrecognized genres.
///
/// https://mutagen-specs.readthedocs.io/en/latest/id3/id3v1-genres.html
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ID3Genres {
    Blues,
    ClassicRock,
    Country,
    Dance,
    Disco,
    Funk,
    Grunge,
    HipHop,
    Jazz,
    Metal,
    NewAge,
    Oldies,
    Other,
    Pop,
    RAndB,
    Rap,
    Reggae,
    Rock,
    Techno,
    Industrial,
    Alternative,
    Ska,
    DeathMetal,
    Pranks,
    Soundtrack,
    EuroTechno,
    Ambient,
    TripHop,
    Vocal,
    JazzAndFunk,
    Fusion,
    Trance,
    Classical,
    Instrumental,
    Acid,
    House,
    Game,
    SoundClip,
    Gospel,
    Noise,
    AltRock,
    Bass,
    Soul,
    Punk,
    Space,
    Meditative,
    InstrumentalPop,
    InstrumentalRock,
    Ethnic,
    Gothic,
    Darkwave,
    TechnoIndustrial,
    Electronic,
    PopFolk,
    Eurodance,
    Dream,
    SouthernRock,
    Comedy,
    Cult,
    GangstaRap,
    Top40,
    ChristianRap,
    PopAndFunk,
    Jungle,
    NativeAmerican,
    Cabaret,
    NewWave,
    Psychedelic,
    Rave,
    Showtunes,
    Trailer,
    LoFi,
    Tribal,
    AcidPunk,
    AcidJazz,
    Polka,
    Retro,
    Musical,
    RockAndRoll,
    HardRock,
    Folk,
    FolkRock,
    NationalFolk,
    Swing,
    FastFusion,
    Bebop,
    Latin,
    Revival,
    Celtic,
    Bluegrass,
    Avantgarde,
    GothicRock,
    ProgressiveRock,
    PsychedelicRock,
    SymphonicRock,
    SlowRock,
    BigBand,
    Chorus,
    EasyListening,
    Acoustic,
    Humour,
    Speech,
    Chanson,
    Opera,
    ChamberMusic,
    Sonata,
    Symphony,
    BootyBass,
    Primus,
    PornGroove,
    Satire,
    SlowJam,
    Club,
    Tango,
    Samba,
    Folklore,
    Ballad,
    PowerBallad,
    RhythmicSoul,
    Freestyle,
    Duet,
    PunkRock,
    DrumSolo,
    ACappella,
    EuroHouse,
    DanceHall,
    Goa,
    DrumAndBass,
    ClubHouse,
    Hardcore,
    Terror,
    Indie,
    BritPop,
    AfroPunk,
    PolskPunk,
    Beat,
    ChristianGangstaRap,
    HeavyMetal,
    BlackMetal,
    Crossover,
    ContemporaryChristian,
    ChristianRock,
    Merengue,
    Salsa,
    ThrashMetal,
    Anime,
    JPop,
    Synthpop,
    Abstract,
    ArtRock,
    Baroque,
    Bhangra,
    BigBeat,
    Breakbeat,
    Chillout,
    Downtempo,
    Dub,
    EBM,
    Eclectic,
    Electro,
    Electroclash,
    Emo,
    Experimental,
    Garage,
    Global,
    IDM,
    Illbient,
    IndustroGoth,
    JamBand,
    Krautrock,
    Leftfield,
    Lounge,
    MathRock,
    NewRomantic,
    NuBreakz,
    PostPunk,
    PostRock,
    Psytrance,
    Shoegaze,
    SpaceRock,
    TropRock,
    WorldMusic,
    Neoclassical,
    Audiobook,
    AudioTheatre,
    NeueDeutscheWelle,
    Podcast,
    IndieRock,
    GFunk,
    Dubstep,
    GarageRock,
    Psybient,
    Unknown,
}

/// The ID3 genre code to genre mapping table
const GENRE_TABLE: [ID3Genres; 192] = [
    ID3Genres::Blues,
    ID3Genres::ClassicRock,
    ID3Genres::Country,
    ID3Genres::Dance,
    ID3Genres::Disco,
    ID3Genres::Funk,
    ID3Genres::Grunge,
    ID3Genres::HipHop,
    ID3Genres::Jazz,
    ID3Genres::Metal,
    ID3Genres::NewAge,
    ID3Genres::Oldies,
    ID3Genres::Other,
    ID3Genres::Pop,
    ID3Genres::RAndB,
    ID3Genres::Rap,
    ID3Genres::Reggae,
    ID3Genres::Rock,
    ID3Genres::Techno,
    ID3Genres::Industrial,
    ID3Genres::Alternative,
    ID3Genres::Ska,
    ID3Genres::DeathMetal,
    ID3Genres::Pranks,
    ID3Genres::Soundtrack,
    ID3Genres::EuroTechno,
    ID3Genres::Ambient,
    ID3Genres::TripHop,
    ID3Genres::Vocal,
    ID3Genres::JazzAndFunk,
    ID3Genres::Fusion,
    ID3Genres::Trance,
    ID3Genres::Classical,
    ID3Genres::Instrumental,
    ID3Genres::Acid,
    ID3Genres::House,
    ID3Genres::Game,
    ID3Genres::SoundClip,
    ID3Genres::Gospel,
    ID3Genres::Noise,
    ID3Genres::AltRock,
    ID3Genres::Bass,
    ID3Genres::Soul,
    ID3Genres::Punk,
    ID3Genres::Space,
    ID3Genres::Meditative,
    ID3Genres::InstrumentalPop,
    ID3Genres::InstrumentalRock,
    ID3Genres::Ethnic,
    ID3Genres::Gothic,
    ID3Genres::Darkwave,
    ID3Genres::TechnoIndustrial,
    ID3Genres::Electronic,
    ID3Genres::PopFolk,
    ID3Genres::Eurodance,
    ID3Genres::Dream,
    ID3Genres::SouthernRock,
    ID3Genres::Comedy,
    ID3Genres::Cult,
    ID3Genres::GangstaRap,
    ID3Genres::Top40,
    ID3Genres::ChristianRap,
    ID3Genres::PopAndFunk,
    ID3Genres::Jungle,
    ID3Genres::NativeAmerican,
    ID3Genres::Cabaret,
    ID3Genres::NewWave,
    ID3Genres::Psychedelic,
    ID3Genres::Rave,
    ID3Genres::Showtunes,
    ID3Genres::Trailer,
    ID3Genres::LoFi,
    ID3Genres::Tribal,
    ID3Genres::AcidPunk,
    ID3Genres::AcidJazz,
    ID3Genres::Polka,
    ID3Genres::Retro,
    ID3Genres::Musical,
    ID3Genres::RockAndRoll,
    ID3Genres::HardRock,
    ID3Genres::Folk,
    ID3Genres::FolkRock,
    ID3Genres::NationalFolk,
    ID3Genres::Swing,
    ID3Genres::FastFusion,
    ID3Genres::Bebop,
    ID3Genres::Latin,
    ID3Genres::Revival,
    ID3Genres::Celtic,
    ID3Genres::Bluegrass,
    ID3Genres::Avantgarde,
    ID3Genres::GothicRock,
    ID3Genres::ProgressiveRock,
    ID3Genres::PsychedelicRock,
    ID3Genres::SymphonicRock,
    ID3Genres::SlowRock,
    ID3Genres::BigBand,
    ID3Genres::Chorus,
    ID3Genres::EasyListening,
    ID3Genres::Acoustic,
    ID3Genres::Humour,
    ID3Genres::Speech,
    ID3Genres::Chanson,
    ID3Genres::Opera,
    ID3Genres::ChamberMusic,
    ID3Genres::Sonata,
    ID3Genres::Symphony,
    ID3Genres::BootyBass,
    ID3Genres::Primus,
    ID3Genres::PornGroove,
    ID3Genres::Satire,
    ID3Genres::SlowJam,
    ID3Genres::Club,
    ID3Genres::Tango,
    ID3Genres::Samba,
    ID3Genres::Folklore,
    ID3Genres::Ballad,
    ID3Genres::PowerBallad,
    ID3Genres::RhythmicSoul,
    ID3Genres::Freestyle,
    ID3Genres::Duet,
    ID3Genres::PunkRock,
    ID3Genres::DrumSolo,
    ID3Genres::ACappella,
    ID3Genres::EuroHouse,
    ID3Genres::DanceHall,
    ID3Genres::Goa,
    ID3Genres::DrumAndBass,
    ID3Genres::ClubHouse,
    ID3Genres::Hardcore,
    ID3Genres::Terror,
    ID3Genres::Indie,
    ID3Genres::BritPop,
    ID3Genres::AfroPunk,
    ID3Genres::PolskPunk,
    ID3Genres::Beat,
    ID3Genres::ChristianGangstaRap,
    ID3Genres::HeavyMetal,
    ID3Genres::BlackMetal,
    ID3Genres::Crossover,
    ID3Genres::ContemporaryChristian,
    ID3Genres::ChristianRock,
    ID3Genres::Merengue,
    ID3Genres::Salsa,
    ID3Genres::ThrashMetal,
    ID3Genres::Anime,
    ID3Genres::JPop,
    ID3Genres::Synthpop,
    ID3Genres::Abstract,
    ID3Genres::ArtRock,
    ID3Genres::Baroque,
    ID3Genres::Bhangra,
    ID3Genres::BigBeat,
    ID3Genres::Breakbeat,
    ID3Genres::Chillout,
    ID3Genres::Downtempo,
    ID3Genres::Dub,
    ID3Genres::EBM,
    ID3Genres::Eclectic,
    ID3Genres::Electro,
    ID3Genres::Electroclash,
    ID3Genres::Emo,
    ID3Genres::Experimental,
    ID3Genres::Garage,
    ID3Genres::Global,
    ID3Genres::IDM,
    ID3Genres::Illbient,
    ID3Genres::IndustroGoth,
    ID3Genres::JamBand,
    ID3Genres::Krautrock,
    ID3Genres::Leftfield,
    ID3Genres::Lounge,
    ID3Genres::MathRock,
    ID3Genres::NewRomantic,
    ID3Genres::NuBreakz,
    ID3Genres::PostPunk,
    ID3Genres::PostRock,
    ID3Genres::Psytrance,
    ID3Genres::Shoegaze,
    ID3Genres::SpaceRock,
    ID3Genres::TropRock,
    ID3Genres::WorldMusic,
    ID3Genres::Neoclassical,
    ID3Genres::Audiobook,
    ID3Genres::AudioTheatre,
    ID3Genres::NeueDeutscheWelle,
    ID3Genres::Podcast,
    ID3Genres::IndieRock,
    ID3Genres::GFunk,
    ID3Genres::Dubstep,
    ID3Genres::GarageRock,
    ID3Genres::Psybient,
];

impl From<u8> for ID3Genres {
    fn from(genre: u8) -> Self {
        if (genre as usize) < GENRE_TABLE.len() {
            GENRE_TABLE[genre as usize]
        } else {
            ID3Genres::Unknown
        }
    }
}

impl From<ID3Genres> for u8 {
    fn from(genre: ID3Genres) -> Self {
        GENRE_TABLE
            .iter()
            .enumerate()
            .filter(|(_, t)| genre == **t)
            .map(|(i, _)| i)
            .next()
            .unwrap_or(255) as u8
    }
}

impl From<ID3Genres> for String {
    fn from(genre: ID3Genres) -> Self {
        match genre {
            ID3Genres::Blues => "Blues",
            ID3Genres::ClassicRock => "Classic Rock",
            ID3Genres::Country => "Country",
            ID3Genres::Dance => "Dance",
            ID3Genres::Disco => "Disco",
            ID3Genres::Funk => "Funk",
            ID3Genres::Grunge => "Grunge",
            ID3Genres::HipHop => "Hip-Hop",
            ID3Genres::Jazz => "Jazz",
            ID3Genres::Metal => "Metal",
            ID3Genres::NewAge => "New Age",
            ID3Genres::Oldies => "Oldies",
            ID3Genres::Other => "Other",
            ID3Genres::Pop => "Pop",
            ID3Genres::RAndB => "R&B",
            ID3Genres::Rap => "Rap",
            ID3Genres::Reggae => "Reggae",
            ID3Genres::Rock => "Rock",
            ID3Genres::Techno => "Techno",
            ID3Genres::Industrial => "Industrial",
            ID3Genres::Alternative => "Alternative",
            ID3Genres::Ska => "Ska",
            ID3Genres::DeathMetal => "Death Metal",
            ID3Genres::Pranks => "Pranks",
            ID3Genres::Soundtrack => "Soundtrack",
            ID3Genres::EuroTechno => "Euro-Techno",
            ID3Genres::Ambient => "Ambient",
            ID3Genres::TripHop => "Trip-Hop",
            ID3Genres::Vocal => "Vocal",
            ID3Genres::JazzAndFunk => "Jazz+Funk",
            ID3Genres::Fusion => "Fusion",
            ID3Genres::Trance => "Trance",
            ID3Genres::Classical => "Classical",
            ID3Genres::Instrumental => "Instrumental",
            ID3Genres::Acid => "Acid",
            ID3Genres::House => "House",
            ID3Genres::Game => "Game",
            ID3Genres::SoundClip => "Sound Clip",
            ID3Genres::Gospel => "Gospel",
            ID3Genres::Noise => "Noise",
            ID3Genres::AltRock => "Alt. Rock",
            ID3Genres::Bass => "Bass",
            ID3Genres::Soul => "Soul",
            ID3Genres::Punk => "Punk",
            ID3Genres::Space => "Space",
            ID3Genres::Meditative => "Meditative",
            ID3Genres::InstrumentalPop => "Instrumental Pop",
            ID3Genres::InstrumentalRock => "Instrumental Rock",
            ID3Genres::Ethnic => "Ethnic",
            ID3Genres::Gothic => "Gothic",
            ID3Genres::Darkwave => "Darkwave",
            ID3Genres::TechnoIndustrial => "Techno-Industrial",
            ID3Genres::Electronic => "Electronic",
            ID3Genres::PopFolk => "Pop-Folk",
            ID3Genres::Eurodance => "Eurodance",
            ID3Genres::Dream => "Dream",
            ID3Genres::SouthernRock => "Southern Rock",
            ID3Genres::Comedy => "Comedy",
            ID3Genres::Cult => "Cult",
            ID3Genres::GangstaRap => "Gangsta Rap",
            ID3Genres::Top40 => "Top 40",
            ID3Genres::ChristianRap => "Christian Rap",
            ID3Genres::PopAndFunk => "Pop/Funk",
            ID3Genres::Jungle => "Jungle",
            ID3Genres::NativeAmerican => "Native American",
            ID3Genres::Cabaret => "Cabaret",
            ID3Genres::NewWave => "New Wave",
            ID3Genres::Psychedelic => "Psychedelic",
            ID3Genres::Rave => "Rave",
            ID3Genres::Showtunes => "Showtunes",
            ID3Genres::Trailer => "Trailer",
            ID3Genres::LoFi => "Lo-Fi",
            ID3Genres::Tribal => "Tribal",
            ID3Genres::AcidPunk => "Acid Punk",
            ID3Genres::AcidJazz => "Acid Jazz",
            ID3Genres::Polka => "Polka",
            ID3Genres::Retro => "Retro",
            ID3Genres::Musical => "Musical",
            ID3Genres::RockAndRoll => "Rock & Roll",
            ID3Genres::HardRock => "Hard Rock",
            ID3Genres::Folk => "Folk",
            ID3Genres::FolkRock => "Folk-Rock",
            ID3Genres::NationalFolk => "National Folk",
            ID3Genres::Swing => "Swing",
            ID3Genres::FastFusion => "Fast-Fusion",
            ID3Genres::Bebop => "Bebop",
            ID3Genres::Latin => "Latin",
            ID3Genres::Revival => "Revival",
            ID3Genres::Celtic => "Celtic",
            ID3Genres::Bluegrass => "Bluegrass",
            ID3Genres::Avantgarde => "Avantgarde",
            ID3Genres::GothicRock => "Gothic Rock",
            ID3Genres::ProgressiveRock => "Progressive Rock",
            ID3Genres::PsychedelicRock => "Psychedelic Rock",
            ID3Genres::SymphonicRock => "Symphonic Rock",
            ID3Genres::SlowRock => "Slow Rock",
            ID3Genres::BigBand => "Big Band",
            ID3Genres::Chorus => "Chorus",
            ID3Genres::EasyListening => "Easy Listening",
            ID3Genres::Acoustic => "Acoustic",
            ID3Genres::Humour => "Humour",
            ID3Genres::Speech => "Speech",
            ID3Genres::Chanson => "Chanson",
            ID3Genres::Opera => "Opera",
            ID3Genres::ChamberMusic => "Chamber Music",
            ID3Genres::Sonata => "Sonata",
            ID3Genres::Symphony => "Symphony",
            ID3Genres::BootyBass => "Booty Bass",
            ID3Genres::Primus => "Primus",
            ID3Genres::PornGroove => "Porn Groove",
            ID3Genres::Satire => "Satire",
            ID3Genres::SlowJam => "Slow Jam",
            ID3Genres::Club => "Club",
            ID3Genres::Tango => "Tango",
            ID3Genres::Samba => "Samba",
            ID3Genres::Folklore => "Folklore",
            ID3Genres::Ballad => "Ballad",
            ID3Genres::PowerBallad => "Power Ballad",
            ID3Genres::RhythmicSoul => "Rhythmic Soul",
            ID3Genres::Freestyle => "Freestyle",
            ID3Genres::Duet => "Duet",
            ID3Genres::PunkRock => "Punk Rock",
            ID3Genres::DrumSolo => "Drum Solo",
            ID3Genres::ACappella => "A Cappella",
            ID3Genres::EuroHouse => "Euro-House",
            ID3Genres::DanceHall => "Dance Hall",
            ID3Genres::Goa => "Goa",
            ID3Genres::DrumAndBass => "Drum & Bass",
            ID3Genres::ClubHouse => "Club-House",
            ID3Genres::Hardcore => "Hardcore",
            ID3Genres::Terror => "Terror",
            ID3Genres::Indie => "Indie",
            ID3Genres::BritPop => "BritPop",
            ID3Genres::AfroPunk => "Afro-Punk",
            ID3Genres::PolskPunk => "Polsk Punk",
            ID3Genres::Beat => "Beat",
            ID3Genres::ChristianGangstaRap => "Christian Gangsta Rap",
            ID3Genres::HeavyMetal => "Heavy Metal",
            ID3Genres::BlackMetal => "Black Metal",
            ID3Genres::Crossover => "Crossover",
            ID3Genres::ContemporaryChristian => "Contemporary Christian",
            ID3Genres::ChristianRock => "Christian Rock",
            ID3Genres::Merengue => "Merengue",
            ID3Genres::Salsa => "Salsa",
            ID3Genres::ThrashMetal => "Thrash Metal",
            ID3Genres::Anime => "Anime",
            ID3Genres::JPop => "JPop",
            ID3Genres::Synthpop => "Synthpop",
            ID3Genres::Abstract => "Abstract",
            ID3Genres::ArtRock => "Art Rock",
            ID3Genres::Baroque => "Baroque",
            ID3Genres::Bhangra => "Bhangra",
            ID3Genres::BigBeat => "Big Beat",
            ID3Genres::Breakbeat => "Breakbeat",
            ID3Genres::Chillout => "Chillout",
            ID3Genres::Downtempo => "Downtempo",
            ID3Genres::Dub => "Dub",
            ID3Genres::EBM => "EBM",
            ID3Genres::Eclectic => "Eclectic",
            ID3Genres::Electro => "Electro",
            ID3Genres::Electroclash => "Electroclash",
            ID3Genres::Emo => "Emo",
            ID3Genres::Experimental => "Experimental",
            ID3Genres::Garage => "Garage",
            ID3Genres::Global => "Global",
            ID3Genres::IDM => "IDM",
            ID3Genres::Illbient => "Illbient",
            ID3Genres::IndustroGoth => "Industro-Goth",
            ID3Genres::JamBand => "Jam Band",
            ID3Genres::Krautrock => "Krautrock",
            ID3Genres::Leftfield => "Leftfield",
            ID3Genres::Lounge => "Lounge",
            ID3Genres::MathRock => "Math Rock",
            ID3Genres::NewRomantic => "New Romantic",
            ID3Genres::NuBreakz => "Nu-Breakz",
            ID3Genres::PostPunk => "Post-Punk",
            ID3Genres::PostRock => "Post-Rock",
            ID3Genres::Psytrance => "Psytrance",
            ID3Genres::Shoegaze => "Shoegaze",
            ID3Genres::SpaceRock => "Space Rock",
            ID3Genres::TropRock => "Trop Rock",
            ID3Genres::WorldMusic => "World Music",
            ID3Genres::Neoclassical => "Neoclassical",
            ID3Genres::Audiobook => "Audiobook",
            ID3Genres::AudioTheatre => "Audio Theatre",
            ID3Genres::NeueDeutscheWelle => "Neue Deutsche Welle",
            ID3Genres::Podcast => "Podcast",
            ID3Genres::IndieRock => "Indie Rock",
            ID3Genres::GFunk => "G-Funk",
            ID3Genres::Dubstep => "Dubstep",
            ID3Genres::GarageRock => "Garage Rock",
            ID3Genres::Psybient => "Psybient",
            ID3Genres::Unknown => "?",
        }
        .to_string()
    }
}

/// The errors that can occur when loading an ID3 tag
pub enum ID3LoadError {
    NoID3Tag,
    IOError(io::Error),
    DecodeError(str::Utf8Error),
    YearError(std::num::ParseIntError, String),
}

impl From<ID3LoadError> for String {
    fn from(err: ID3LoadError) -> Self {
        match err {
            ID3LoadError::NoID3Tag => "No ID3 'TAG' magic in file".to_string(),
            ID3LoadError::IOError(err) => format!("IO error: {}", err),
            ID3LoadError::DecodeError(err) => format!("Decoding error: {}", err),
            ID3LoadError::YearError(err, year) => {
                format!("Could not parse year {:#?}: {}", year, err)
            }
        }
    }
}

/// The ID3 metadata tags stored on a file
#[derive(Debug)]
pub struct ID3 {
    title: String,
    artist: String,
    album: String,
    year: u16,
    comment: String,
    track: Option<u8>,
    genre: ID3Genres,
}

impl ID3 {
    /// Creates a new empty ID3 tag with default values
    pub fn new() -> Self {
        ID3 {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            // The spec doesn't say how the year is padded, so we ensure that
            // it's somewhere in the range [1000, 9999] to avoid ambiguity
            year: 1000,
            comment: String::new(),
            track: None,
            genre: ID3Genres::Unknown,
        }
    }

    /// Gets the ID3 title of the tags
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Gets the ID3 artist of the tags
    pub fn artist(&self) -> &str {
        &self.artist
    }

    /// Gets the ID3 album of the tags
    pub fn album(&self) -> &str {
        &self.album
    }

    /// Gets the ID3 album of the tags
    pub fn year(&self) -> u16 {
        self.year
    }

    /// Gets the ID3 album of the tags
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// Gets the ID3 track of the tags
    pub fn track(&self) -> &Option<u8> {
        &self.track
    }

    /// Gets the ID3 track of the tags
    pub fn genre(&self) -> ID3Genres {
        self.genre
    }

    /*
     * ID3 has a basic format with a few fixed-width fields. The whole structure
     * adds up to 128 bytes, with all free-form text being padded out to its specified
     * field length with NULs (or trimmed if they are too long)
     *
     * Size Offset Description
     * ---- ------ -------------------------------------------------------------------------------------------
     *   3       0 Must be the ASCII text "TAG"
     *  30       3 The title
     *  30      33 The artist
     *  30      63 The album
     *   4      93 The year
     *  28      97 The comment
     *   1     125 A zero-byte. If this is non-zero then the comment is 30 bytes instead and we have no track.
     *   1     126 The track number
     *   1     127 The genre number
     */

    /// Generates a new ID3 struct from a stream
    pub fn from_stream<T: Read + Seek>(stream: &mut T) -> Result<Self, ID3LoadError> {
        if let Err(err) = stream.seek(io::SeekFrom::End(-128)) {
            return Err(ID3LoadError::IOError(err));
        }

        let mut buffer = [0; 128];
        if let Err(err) = stream.read_exact(&mut buffer) {
            return Err(ID3LoadError::IOError(err));
        }

        // "TAG" check
        if &buffer[..3] != [84, 65, 71] {
            return Err(ID3LoadError::NoID3Tag);
        }

        let title_bytes = buffer[3..33]
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| b.clone())
            .collect::<Vec<_>>();

        let title = match str::from_utf8(&title_bytes) {
            Ok(s) => s.to_string(),
            Err(err) => return Err(ID3LoadError::DecodeError(err)),
        };

        let artist_bytes = buffer[33..63]
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| b.clone())
            .collect::<Vec<_>>();

        let artist = match str::from_utf8(&artist_bytes) {
            Ok(s) => s.to_string(),
            Err(err) => return Err(ID3LoadError::DecodeError(err)),
        };

        let album_bytes = buffer[63..93]
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| b.clone())
            .collect::<Vec<_>>();

        let album = match str::from_utf8(&album_bytes) {
            Ok(s) => s.to_string(),
            Err(err) => return Err(ID3LoadError::DecodeError(err)),
        };

        let track_marker = buffer[125];
        let track_number = buffer[126];
        let genre_number = buffer[127];

        let mut comment_bytes = buffer[97..125]
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| b.clone())
            .collect::<Vec<_>>();

        if track_marker != 0 {
            comment_bytes.push(track_number);
            if track_number != 0 {
                comment_bytes.push(track_number);
            }
        }

        let comment = match str::from_utf8(&comment_bytes) {
            Ok(s) => s.to_string(),
            Err(err) => return Err(ID3LoadError::DecodeError(err)),
        };

        let year = str::from_utf8(&buffer[93..97])
            .or_else(|err| Err(ID3LoadError::DecodeError(err)))
            .and_then(|s| {
                if s == "\x00\x00\x00\x00" {
                    Ok(0)
                } else {
                    u16::from_str_radix(s, 10)
                        .or_else(|err| Err(ID3LoadError::YearError(err, s.to_string())))
                }
            });

        match year {
            Err(err) => return Err(err),
            Ok(year) => Ok(ID3 {
                title,
                artist,
                album,
                year,
                comment,
                track: if track_marker == 0 {
                    None
                } else {
                    Some(track_number)
                },
                genre: genre_number.into(),
            }),
        }
    }

    /// Writes ID3 tags onto a file stream at the current position
    pub fn to_stream<T: Write>(&self, stream: &mut T) -> io::Result<()> {
        if self.year > 9999 {
            return Err(io::Error::new(io::ErrorKind::Other, "Invalid year data"));
        }

        let year_text = format!("{:04}", self.year);

        // Make sure that every field is NUL terminated, since ezstream can
        // crash or produce corrupt tags without this
        stream.write("TAG".as_bytes())?;
        stream.write(&pad_bytes(&self.title, 29))?;
        stream.write(&[0])?;
        stream.write(&pad_bytes(&self.artist, 29))?;
        stream.write(&[0])?;
        stream.write(&pad_bytes(&self.album, 29))?;
        stream.write(&[0])?;
        stream.write(&year_text.as_bytes())?;

        match self.track {
            Some(track) => {
                stream.write(&pad_bytes(&self.comment, 28))?;
                stream.write(&[0, track])?;
            }
            None => {
                stream.write(&pad_bytes(&self.comment, 29))?;
                stream.write(&[0])?;
            }
        };

        let genre_code: u8 = self.genre.into();
        stream.write(&[genre_code])?;
        Ok(())
    }
}

/// Encodes a string into bytes of the given length, either truncating or
/// padding with NUL bytes as necessary
fn pad_bytes(value: &str, length: usize) -> Vec<u8> {
    let mut buffer = value.as_bytes().to_vec();
    buffer.resize(length, 0);
    buffer
}

/// Reads a text announcement and outputs an ID3-tagged MP3 file
pub fn read_text_announcement(
    announcement: &str,
    outputs: &FileOutputs,
    title: &str,
) -> Result<(), String> {
    Command::new("/usr/bin/espeak")
        .arg("-g")
        .arg("15")
        .arg("-w")
        .arg(outputs.mono_wav)
        .arg(announcement)
        .output()
        .or_else(|err| Err(format!("Could not invoke espeak: {}", err)))?;

    Command::new("/usr/bin/sox")
        .arg(outputs.mono_wav)
        .arg("-r")
        .arg("44.1k")
        .arg("-c")
        .arg("2")
        .arg(outputs.stereo_wav)
        .output()
        .or_else(|err| Err(format!("Could not invoke sox: {}", err)))?;

    Command::new("/usr/bin/lame")
        .arg(outputs.stereo_wav)
        .arg(outputs.lame_mp3)
        .output()
        .or_else(|err| Err(format!("Could not invoke lame: {}", err)))?;

    // ID3v1.1 header
    let mut mp3_options = fs::OpenOptions::new();
    let mut mp3 = mp3_options
        .append(true)
        .open(outputs.lame_mp3)
        .or_else(|err| Err(format!("Could not open MP3 file for write: {}", err)))?;

    let mut tag = ID3::new();
    tag.title.push_str(title);
    tag.artist.push_str("shuffled");
    tag.year = 2020;
    tag.album.push_str("shuffled tasks");
    tag.track = Some(1);
    tag.comment.push_str("Generated by shuffled");
    tag.to_stream(&mut mp3)
        .or_else(|err| Err(format!("Could not write ID3: {}", err)))?;

    fs::rename(outputs.lame_mp3, outputs.final_mp3)
        .or_else(|err| Err(format!("Could not move temp MP3 {} to {}: {}",
                                   outputs.lame_mp3.display(),
                                   outputs.final_mp3.display(),
                                   err)))
}

/// Creates a new RNG seeded either from /dev/urandom or the system time
pub fn seeded_random() -> random::Default {
    let (upper_seed, lower_seed) = fs::File::open("/dev/urandom")
        .map(|mut urandom| {
            let mut buffer = [0; 16];
            if let Ok(16) = urandom.read(&mut buffer) {
                let upper = u64::from_le_bytes(buffer[..8].try_into().unwrap());
                let lower = u64::from_le_bytes(buffer[8..].try_into().unwrap());
                (upper, lower)
            } else if let Ok(duration) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                let upper = (duration.as_nanos() >> 64) as u64;
                let lower = duration.as_nanos() as u64;
                (upper, lower)
            } else {
                (12345, 67890)
            }
        })
        .unwrap_or((12345, 67890));

    random::default().seed([upper_seed, lower_seed])
}
