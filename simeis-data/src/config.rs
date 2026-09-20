use std::{path::Path, sync::OnceLock};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::errors::Errcode;

static CONFIG: OnceLock<SimeisConfig> = OnceLock::new();

pub fn get_config<'a>() -> &'a SimeisConfig {
    CONFIG.get().unwrap()
}

pub fn init_default_config() {
    let cfg = SimeisConfig::default();
    let _ = CONFIG.set(cfg);
}

pub fn init_config<'a>(fpath: &Path) -> Result<&'a SimeisConfig, Errcode> {
    let cfg = if !fpath.is_file() {
        println!("Config file is missing, generating one from default configuration...");
        let cfg = SimeisConfig::default();
        let cfg_str = serde_json::to_string_pretty(&cfg).map_err(Errcode::DefaultConfigToJson)?;
        std::fs::write(fpath, cfg_str).map_err(Errcode::WriteDefaultConfig)?;
        cfg
    } else {
        let s = std::fs::read_to_string(fpath).map_err(Errcode::ReadConfigFile)?;
        let cfg: SimeisConfig__deser = serde_json::from_str(&s).map_err(Errcode::LoadConfigJson)?;
        SimeisConfig::from(cfg)
    };
    CONFIG.set(cfg).unwrap();
    Ok(CONFIG.get().unwrap())
}

fn get_val<T: DeserializeOwned>(val: Option<T>, envn: &'static str, defval: T) -> T {
    if !envn.is_empty() {
        if let Ok(v) = std::env::var(envn) {
            return serde_json::from_str(&v).expect("Unable to load env var value");
        }
    }
    if let Some(v) = val {
        v
    } else {
        defval
    }
}

macro_rules! config_fields {
    ($struct_name:ident; $($name:ident: $t:ty => $envn:expr => $defval:expr $(,)*),*) => {
        paste::paste!{
        #[derive(Serialize, Debug)]
        pub struct [< $struct_name >] {
            $( pub $name: $t, )*
        }

        #[derive(Deserialize, Default)]
        #[allow(non_camel_case_types)]
        pub struct [< $struct_name __deser >] {
            $(
                #[serde(default)]
                $name: Option<$t>,
            )*
        }

        impl [< $struct_name __deser >] {
            pub fn resolve(self) -> $struct_name {
                $struct_name {
                $(
                    $name: get_val(self.$name, $envn, $defval),
                )*
                }
            }
        }
        }
    };
}
macro_rules! main_config {
    ($name:ident; $($n:ident: $cfgty:ty $(,)*),*) => {
        paste::paste!{
        #[derive(Deserialize, Default)]
        #[allow(non_camel_case_types)]
        pub struct [< $name __deser >] {
            $(
                #[serde(default)]
                $n: [< $cfgty __deser >],
            )*
        }

        impl From<[< $name __deser >]> for $name {
            fn from(value: [< $name __deser >]) -> $name {
                $name {
                    $(
                        $n: value.$n.resolve(),
                    )*
                }
            }
        }
        }

        #[derive(Serialize, Debug)]
        pub struct $name {
            $(
                pub $n: $cfgty,
            )*
        }

        paste::paste!{
        impl Default for $name {
            fn default() -> $name {
                let d = [< $name __deser >]::default();
                $name::from(d)
            }
        }
        }
    };
}

config_fields!(SimeisServerConfig;
    port: u16 => "SIMEIS_PORT" => 8080,
    workers: usize => "SIMEIS_WORKERS" => 64,
    bind_ip: String => "" => "0.0.0.0".to_string(),
    logger_env_name: String => "" => "SIMEIS_LOG".to_string(),
    token_file: String => "SIMEIS_TOKEN_FILE" => ".simeis-admin-token".to_string(),
);

config_fields!(SimeisGameConfig;
    shards: usize => "SIMEIS_LOCKS_SHARDS" => 5,
    player_init_money: f64 => "SIMEIS_PLAYER_INIT_MONEY" => 72000.0,
    sector_size: u32 => "" => 2000,
    planets_per_sector: usize => "" => 12,
    station_fplanet_dist: f64 => "" => 300.0,
);

config_fields!(SimeisUpgradeConfig;
    industry_unitupg_powf_div: f64 => "" => 75.0,
    module_upg_powf_div: f64 => "" => 75.0,
);

config_fields!(SimeisShipConfig;
    fuel_tank_cap_price: f64 => "" => 30.0,
    cargo_cap_price: f64 => "" => 20.0,
    hull_resis_price: f64 => "" => 9.0,
    reactor_rank_price: f64 => "" => 5000.0,
    shield_price: f64 => "" => 3500.0,
    default_pow_attr: u8 => "" => 128,

    module_base_price: f64 => "" => 4500.0,
    module_base_extraction_mult: f64 => "" => 50.0,
    module_exrate_diff_fact: f64 => "" => 1.5,
    module_exrate_fact: f64 => "" => 0.6,
    module_rank_weight: f64 => "" => 2.0,

    hull_usage_rate: f64 => "" => 30.0 / 1000.0,
    reactor_speed_rate: f64 => "" => 30.0,
    shield_rank_log: f64 => "" => 4.0,
    pilot_fuel_share: u8 => "" => 7,
    pilot_rank_speed_powf: f64 => "" => 1.2,
);
config_fields!(SimeisStationConfig;
    cargo_base_price: f64 => "" => 20.0,
    cargo_price_incdiv: f64 => "" => 100.0,
    init_cargo: f64 => "" => 2000.0,
);
config_fields!(SimeisCrewConfig;
    wage_rank_powf: f64 => "" => 0.85,
    wage_next_rank_powf: f64 => "" => 1.4,
    wage_next_rank_mult: f64 => "" => 1500.0,

    base_op_wage: f64 => "" => 0.75,
    pilot_wage_mult: f64 => "" => 3.5,
    trader_wage_mult: f64 => "" => 5.0,
    solider_wage_mult: f64 => "" => 2.5,
);

config_fields!(SimeisMarketConfig;
    max_avg_ampl: f64 => "" => 3.2 / 100.0,
    std_div: f64 => "" => 1.08,
    change_sec: f64 => "" => 10.0,
    base_fee_rate: f64 => "" => 25.0 / 100.0,
    fee_rate_dev_powf: f64 => "" => 0.95,
    upd_price_proba: f64 => "" => 0.30,
);

config_fields!(SimeisIndustryConfig;
    simple_bprice_mult: f64 => "" => 1.08,
    simple_unit_price: f64 => "" => 8000.0,
    advanced_bprice_mult: f64 => "" => 1.75,
    advanced_unit_price: f64 => "" => 18000.0,

    simple_industry_tier1_resources_req: f64 => "" => 1.5,
    simple_res2rq: f64 => "" => 0.2,
    simple_res3rq: f64 => "" => 1.25,
    simple_res4rq: f64 => "" => 0.4,

    advanced_industry_tier1_resources_req: f64 => "" => 7.5,
    advanced_res2rq: f64 => "" => 0.4,
    advanced_res3rq: f64 => "" => 0.2,
);
config_fields!(SimeisResourcesConfig;
    tier1_price: f64 => "" => 6.0,
    tier2_price_mult: f64 => "" => 2.0,
    tier3_price_mult: f64 => "" => 5.0,
    tier4_price_mult: f64 => "" => 8.0,

    tier1_volume: f64 => "" => 2.5,
    tier2_volume: f64 => "" => 1.9,
    tier3_volume: f64 => "" => 1.2,
    tier4_volume: f64 => "" => 0.85,

    fuel_price: f64 => "" => 12.0,
    hull_price: f64 => "" => 1.0,
    fuel_volume: f64 => "" => 6.0,
    hull_volume: f64 => "" => 0.75,

    tier1_difficulty: f64 => "" => 2.0,
    tier2_difficulty_mult: f64 => "" => 2.0,
    tier3_difficulty_mult: f64 => "" => 5.0,
    tier4_difficulty_mult: f64 => "" => 8.0,

    tier1_min_rank: u8 => "" => 0,
    tier2_min_rank: u8 => "" => 2,
    tier3_min_rank: u8 => "" => 4,
    tier4_min_rank: u8 => "" => 6,
);

main_config!(SimeisConfig;
    server: SimeisServerConfig,
    game: SimeisGameConfig,
    resources: SimeisResourcesConfig,
    industry: SimeisIndustryConfig,
    upgrade: SimeisUpgradeConfig,
    station: SimeisStationConfig,
    ship: SimeisShipConfig,
    crew: SimeisCrewConfig,
    market: SimeisMarketConfig,
);
