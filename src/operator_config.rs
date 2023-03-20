use config::{Config, ConfigError, File, FileFormat};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct Settings {
    pub namespaces: Vec<String>,
    pub default_config: DefaultSeroConfig,
}

#[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct DefaultSeroConfig {
    pub image: String,
    pub inject: bool,
    pub protocol: String,
    pub port: i64,
    pub timeout_forward_ms: i64,
    pub timeout_scale_up_ms: i64,
    pub timeout_scale_down_ms: i64,
}

const CONFIG_FILE_PREFIX: &str = "./config.yaml";

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let s = Config::builder()
        .add_source(File::new(CONFIG_FILE_PREFIX, FileFormat::Yaml))
        // .add_source(File::new(CONFIG_FILE_PREFIX, FileFormat::Toml))
        // .add_source(File::new(CONFIG_FILE_PREFIX, FileFormat::Json))
        // todo: set default struct
        .build();
        
        s.unwrap().try_deserialize::<Settings>()
    }
  }
  

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            namespaces: vec![],
            default_config: DefaultSeroConfig {
                image: String::from("ghcr.io/fluktuid/sero.rs:latest"),
                inject: true,
                protocol: String::from("TCP"),
                port: 80,
                timeout_forward_ms: 2000,
                timeout_scale_up_ms: 7000,
                timeout_scale_down_ms: 7000,
            }
        }
    }
}
