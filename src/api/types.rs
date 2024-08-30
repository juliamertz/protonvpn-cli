use clap::ValueEnum;
use serde::{Deserialize, Deserializer, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Default, Deserialize, Serialize, Clone, ValueEnum)]
pub enum Tier {
    Free,
    #[default]
    Premium,
    All,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Features: u8 {
        const SecureCore  = 1 << 0;  // 1
        const Tor         = 1 << 1;  // 2
        const P2P         = 1 << 2;  // 4
        const Streaming   = 1 << 3;  // 8
        const Ipv6        = 1 << 4;  // 16
    }
}

impl Serialize for Features {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.bits())
    }
}

impl<'de> Deserialize<'de> for Features {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Features::from_bits(value).ok_or_else(|| {
            serde::de::Error::custom(format!("Invalid value for Features: {}", value))
        })
    }
}

// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct Location {
//     #[serde(rename = "Lat")]
//     lat: f64,
//
//     #[serde(rename = "Long")]
//     long: f64,
// }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Server {
    #[serde(rename = "EntryIP")]
    pub entry_ip: Ipv4Addr,
    // #[serde(rename = "ExitIP")]
    // pub exit_ip: Ipv4Addr,

    // #[serde(rename = "Domain")]
    // pub domain: String,

    // #[serde(rename = "ID")]
    // pub id: String,

    // #[serde(rename = "Label")]
    // pub label: String,

    // #[serde(rename = "X25519PublicKey")]
    // pub x25519_public_key: String,

    // #[serde(rename = "Generation")]
    // pub generation: u8,

    // #[serde(rename = "Status")]
    // pub status: u8,
    //
    // #[serde(rename = "ServicesDown")]
    // pub services_down: u8,

    // #[serde(rename = "ServicesDownReason")]
    // pub services_down_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogicalServer {
    #[serde(rename = "Name")]
    pub name: String,

    // #[serde(rename = "EntryCountry")]
    // pub entry_country: Country,
    #[serde(rename = "ExitCountry")]
    pub exit_country: Country,

    // #[serde(rename = "Domain")]
    // pub domain: String,
    #[serde(rename = "Tier")]
    pub tier: u8,

    #[serde(rename = "Features")]
    pub features: Features,

    // #[serde(rename = "Region")]
    // pub region: Option<String>,

    // #[serde(rename = "City")]
    // pub city: Option<String>,
    #[serde(rename = "Score")]
    pub score: f64,

    // #[serde(rename = "HostCountry")]
    // pub host_country: Option<Country>,
    #[serde(rename = "ID")]
    pub id: String,

    // #[serde(rename = "Location")]
    // pub location: Location,

    // TODO: Filter out servers where status is not OK
    #[serde(rename = "Status")]
    pub status: u8,

    #[serde(rename = "Servers")]
    pub servers: Vec<Server>,

    #[serde(rename = "Load")]
    pub load: u8,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum Country {
    JP,
    FR,
    ES,
    NL,
    DE,
    CA,
    UK,
    US,
    IT,
    FI,
    IE,
    AT,
    PL,
    SG,
    NO,
    IN,
    DK,
    CZ,
    BE,
    LU,
    PT,
    IL,
    RU,
    AU,
    EE,
    UA,
    LV,
    SK,
    RS,
    GR,
    LT,
    AE,
    MX,
    SI,
    MY,
    NZ,
    HK,
    MD,
    AR,
    HU,
    TW,
    CY,
    NG,
    RO,
    KR,
    PH,
    KH,
    EG,
    VN,
    BR,
    PR,
    GE,
    SE,
    ZA,
    TH,
    CH,
    IS,
    BG,
    TR,
    CO,
    CL,
    PE,
    EC,
    HR,
    MT,
    CR,
    MM,
    MK,
    ID,
    PK,
    MA,
    LK,
    BD,
    BY,
    NP,
    VE,
    SV,
    MZ,
    RW,
    SN,
    TG,
    DZ,
    TD,
    SS,
    MR,
    AZ,
    QA,
    AL,
    AO,
    BH,
    KM,
    ER,
    ET,
    IQ,
    JO,
    KE,
    KZ,
    MU,
    ME,
    SD,
    SO,
    TM,
    UZ,
    KW,
    LY,
    SA,
    SY,
    TJ,
    TN,
    AF,
    YE,
    BT,
}
