// Copyright Judica, Inc 2021
//
// This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at https://mozilla.org/MPL/2.0/.

use bitcoin::util::bip32::ExtendedPubKey;
use bitcoincore_rpc_async as rpc;

use directories::BaseDirs;
use emulator_connect::connections::federated::FederatedEmulatorConnection;
use emulator_connect::connections::hd::HDOracleEmulatorConnection;
use emulator_connect::CTVEmulator;
use schemars::JsonSchema;
use serde::*;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::{io::BufReader, runtime::Handle};
/// EmulatorConfig is used to determine how this sapio-cli instance should stub
/// out CTV. Emulators are specified by EPK and interface address. Threshold
/// should be <= emulators.len().
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct EmulatorConfig {
    /// if the emulator should be used or not. We tag explicitly for convenience
    /// in the config file format.
    pub enabled: bool,
    #[schemars(with = "Vec<(String, String)>")]
    pub emulators: Vec<(ExtendedPubKey, String)>,
    /// threshold could be larger than u8, but that seems very unlikely/an error.
    pub threshold: u8,
}

impl EmulatorConfig {
    /// Converts a config instance into an emulator trait object. Intenrally, we
    /// are using a Federated Emulator Connection if emulators.len() > 1, or a
    /// bare HDOracleEmulatorConnection if emulators.len() == 1
    pub fn get_emulator(&self) -> Result<Arc<dyn CTVEmulator>, Box<dyn std::error::Error>> {
        if self.emulators.len() < self.threshold as usize {
            Err(String::from("Too High Thresh"))?;
        } else if self.emulators.len() == 0 {
            Err(String::from("Too High Thresh"))?;
        }
        let _n_emulators = self.emulators.len();
        let rt = Handle::try_current()
            .err()
            .map(|_e| Arc::new(tokio::runtime::Runtime::new().unwrap()));
        let secp = Arc::new(bitcoin::secp256k1::Secp256k1::new());
        let mut it =
            self.emulators
                .iter()
                .map(|(epk, host)| -> Result<_, Box<dyn std::error::Error>> {
                    let handle = Handle::try_current().unwrap_or_else(|_e| {
                        rt.as_ref().expect("must have own runtime").handle().clone()
                    });
                    Ok(HDOracleEmulatorConnection {
                        handle,
                        runtime: rt.clone(),
                        connection: Mutex::new(None),
                        reconnect: host.to_socket_addrs()?.next().unwrap(),
                        root: *epk,
                        secp: secp.clone(),
                    })
                });
        Ok(if self.emulators.len() == 1 {
            Arc::new(it.next().unwrap()?)
        } else {
            Arc::new(FederatedEmulatorConnection::new(
                it.map(|n| -> Result<_, Box<dyn std::error::Error>> {
                    let b: Arc<dyn CTVEmulator> = Arc::new(n?);
                    Ok(b)
                })
                .collect::<Result<Vec<_>, _>>()?,
                self.threshold,
            ))
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(into = "PathBuf")]
#[serde(from = "String")]
struct PathBufWrapped(PathBuf);
impl From<String> for PathBufWrapped {
    fn from(s: String) -> Self {
        PathBufWrapped(s.into())
    }
}
impl Into<PathBuf> for PathBufWrapped {
    fn into(self) -> PathBuf {
        self.0
    }
}
/// Used to serailize/deserialize pathbufs for config
mod pathbuf_serde {
    use serde::*;
    use std::path::PathBuf;
    pub fn serialize<S>(p: &PathBuf, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(p.to_str().unwrap())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(String::deserialize(d)?.into())
    }
}

/// Remote type Derivation for rpc::Auth
/// TODO: Move to the RPC Library?
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(remote = "rpc::Auth")]
pub enum Auth {
    None,
    UserPass(String, String),
    CookieFile(
        #[serde(with = "pathbuf_serde")]
        #[schemars(with = "String")]
        PathBuf,
    ),
}

/// Which Bitcoin Node should Sapio use
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Node {
    pub url: String,
    #[serde(with = "Auth")]
    pub auth: rpc::Auth,
}

/// A configuration for any network (regtest, main, signet, testnet)
/// Only one config may set active = true at a time.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkConfig {
    /// if this is the active config
    pub active: bool,
    pub api_node: Node,
    pub emulator_nodes: Option<EmulatorConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub plugin_map: Option<BTreeMap<String, WasmerCacheHash>>,
}

impl From<WasmerCacheHash> for [u8; 32] {
    fn from(x: WasmerCacheHash) -> Self {
        x.0.into()
    }
}

/// An ID for an uncompiled Plugin Wasm Binary
/// It is serialized as a hex slice.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(try_from = "String", into = "String")]
pub struct WasmerCacheHash([u8; 32]);

use bitcoin::hashes::hex::{FromHex, ToHex};
impl From<WasmerCacheHash> for String {
    fn from(x: WasmerCacheHash) -> Self {
        ToHex::to_hex(&x.0[..])
    }
}

impl TryFrom<String> for WasmerCacheHash {
    type Error = bitcoin::hashes::hex::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        FromHex::from_hex(&s).map(WasmerCacheHash)
    }
}

/// This config has only the currently active network, the other configs get
/// dropped during the ConfigVerifier::try_into.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(try_from = "ConfigVerifier")]
pub struct Config {
    pub active: NetworkConfig,
    pub network: bitcoin::network::constants::Network,
}

impl Config {
    /// reads the user's config file and returns it,
    /// or a different one if the user specified a different file manually.
    ///
    /// if no config is found for the user, creates a file.
    ///
    /// **Race Conditions** This is clearly not safe if multiple edits are
    /// happening on config.json. It is assumed that the user will ensure
    /// writes to config.json are safe.
    pub async fn setup(
        custom_config: Option<&str>,
        typ: &str,
        org: &str,
        proj: &str,
    ) -> Result<Config, Box<dyn std::error::Error>> {
        if let Some(p) = custom_config {
            Ok(serde_json::from_slice(&tokio::fs::read(p).await?[..])?)
        } else {
            let proj = directories::ProjectDirs::from(typ, org, proj)
                .expect("Failed to find config directory");
            let path = proj.config_dir();
            tokio::fs::create_dir_all(path).await?;
            let mut pb = path.to_path_buf();
            pb.push("config.json");
            if let Ok(txt) = tokio::fs::read(&pb).await {
                Ok(serde_json::from_slice(&txt[..])?)
            } else {
                Err("Please Run the configure wizard command to make a config file")?
            }
        }
    }
}

/// This is a deserialization helper which checks the config file for well
/// formedness before processing into an actual config.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigVerifier {
    main: Option<NetworkConfig>,
    testnet: Option<NetworkConfig>,
    signet: Option<NetworkConfig>,
    regtest: Option<NetworkConfig>,
}
impl TryFrom<ConfigVerifier> for Config {
    type Error = ConfigError;

    fn try_from(
        cfg: ConfigVerifier,
    ) -> std::result::Result<Self, <Self as std::convert::TryFrom<ConfigVerifier>>::Error> {
        let network = cfg.get_network()?;
        Ok(Config {
            active: cfg.check()?,
            network,
        })
    }
}

impl From<Config> for ConfigVerifier {
    fn from(c: Config) -> ConfigVerifier {
        let mut res = ConfigVerifier {
            main: None,
            testnet: None,
            signet: None,
            regtest: None,
        };
        match c.network {
            bitcoin::network::constants::Network::Regtest => {
                res.regtest = Some(c.active);
            }
            bitcoin::network::constants::Network::Signet => {
                res.signet = Some(c.active);
            }
            bitcoin::network::constants::Network::Testnet => {
                res.testnet = Some(c.active);
            }
            bitcoin::network::constants::Network::Bitcoin => {
                res.main = Some(c.active);
            }
        };
        res
    }
}

impl ConfigVerifier {
    /// Return the active network
    fn get_network(&self) -> Result<bitcoin::network::constants::Network, ConfigError> {
        match self.get_n() {
            1 => Err(ConfigError::NoActiveConfig),
            3 => Ok(bitcoin::network::constants::Network::Bitcoin),
            11 => Ok(bitcoin::network::constants::Network::Testnet),
            7 => Ok(bitcoin::network::constants::Network::Regtest),
            5 => Ok(bitcoin::network::constants::Network::Signet),
            _ => Err(ConfigError::TooManyActiveNetworks),
        }
    }
    /// This is some... clever... code which assigns a prime number to every
    /// active network and then multiplies them all together.
    ///
    /// The result can then be used to pick which network should be used & verify
    /// that only one network is active at once.
    ///
    /// The alternative is a bit messier unfortunately, but maybe simpler as a refactor.
    fn get_n(&self) -> i32 {
        let v0 = self.main.as_ref().map(|c| 3 * c.active as i32).unwrap_or(1);
        let v1 = self
            .signet
            .as_ref()
            .map(|c| 5 * c.active as i32)
            .unwrap_or(1);
        let v2 = self
            .regtest
            .as_ref()
            .map(|c| 7 * c.active as i32)
            .unwrap_or(1);
        let v3 = self
            .testnet
            .as_ref()
            .map(|c| 11 * c.active as i32)
            .unwrap_or(1);
        v0 * v1 * v2 * v3
    }
    /// Checks the config for correctness and then returns the active config.
    pub fn check(self) -> Result<NetworkConfig, ConfigError> {
        match self.get_n() {
            1 => Err(ConfigError::NoActiveConfig),
            3 => Ok(self.main.unwrap()),
            5 => Ok(self.signet.unwrap()),
            7 => Ok(self.regtest.unwrap()),
            11 => Ok(self.testnet.unwrap()),
            _ => Err(ConfigError::TooManyActiveNetworks),
        }
    }

    pub async fn wizard() -> Result<Self, Box<dyn std::error::Error>> {
        use tokio::io::AsyncBufReadExt;

        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut b = BaseDirs::new()
            .expect("Could Not Determine a Base Directory")
            .home_dir()
            .to_path_buf();
        b.push(".bitcoin");
        b.push("regtest");
        b.push(".cookie");
        let network;
        let mut lines = reader.lines();
        loop {
            println!("Which Network? (main, reg, sig, test): ");
            if let Some(line) = lines.next_line().await? {
                network = match line.trim() {
                    "main" => bitcoin::network::constants::Network::Bitcoin,
                    "reg" => bitcoin::network::constants::Network::Regtest,
                    "sig" => bitcoin::network::constants::Network::Signet,
                    "test" => bitcoin::network::constants::Network::Testnet,
                    _ => {
                        println!("Not a valid option {:?}", line);
                        continue;
                    }
                };
                break;
            }
        }
        let mut url: String;
        loop {
            println!("API Node URL (e.g., http://127.0.0.1:18443): ");
            if let Some(line) = lines.next_line().await? {
                url = line.trim().into();
                if url.len() == 0 {
                    println!("Must enter a username");
                } else {
                    break;
                }
            }
        }
        let using_cookie;
        loop {
            println!("Auth Type (for cookie file, \"cookie\", for username/password \"basic\"): ");
            if let Some(line) = lines.next_line().await? {
                using_cookie = match line.trim().into() {
                    "cookie" => true,
                    "basic" => false,
                    l => {
                        println!("Invalid option {}, type cookie or basic:", l);
                        continue;
                    }
                };
                break;
            }
        }
        let auth = if using_cookie {
            let mut cookie: String;
            loop {
                println!("Cookie file location (e.g., {}): ", b.display());
                if let Some(line) = lines.next_line().await? {
                    cookie = line.trim().into();
                    if cookie.len() == 0 {
                        println!("Must give a cookie file location.");
                        continue;
                    }
                    break;
                }
            }
            rpc::Auth::CookieFile(cookie.into())
        } else {
            let mut username: String;
            loop {
                println!("Username: ");
                if let Some(line) = lines.next_line().await? {
                    username = line.trim().into();
                    if username.len() == 0 {
                        println!("Must enter a username");
                    } else {
                        break;
                    }
                }
            }
            let mut password: String;
            loop {
                println!("Password: ");
                if let Some(line) = lines.next_line().await? {
                    password = line.trim().into();
                    if password.len() == 0 {
                        println!("Must enter a username");
                    } else {
                        break;
                    }
                }
            }
            rpc::Auth::UserPass(username, password)
        };

        println!("Configuration Complete!");
        println!("To configure Emulators/ Plugin Maps please edit manually if desired.");
        println!("Your Configuration:");

        let active = NetworkConfig {
            active: true,
            api_node: Node{url, auth},
            emulator_nodes: Some(EmulatorConfig{
                enabled: false,
                threshold: 1u8,
                emulators: vec![(ExtendedPubKey::from_str("tpubD6NzVbkrYhZ4Wf398td3H8YhWBsXx9Sxa4W3cQWkNW3N3DHSNB2qtPoUMXrA6JNaPxodQfRpoZNE5tGM9iZ4xfUEFRJEJvfs8W5paUagYCE").unwrap(),
                    "example.please.change.this.before.using:8367".into())],
            }),
            plugin_map: None,
        };
        let cv: ConfigVerifier = Config { network, active }.into();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::to_value(cv.clone())?)?
        );

        Ok(cv)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    TooManyActiveNetworks,
    NoActiveConfig,
}
use std::fmt;
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ConfigError {}

impl std::default::Default for ConfigVerifier {
    fn default() -> Self {
        let mut b = BaseDirs::new()
            .expect("Could Not Determine a Base Directory")
            .home_dir()
            .to_path_buf();
        b.push(".bitcoin");
        b.push("regtest");
        b.push(".cookie");
        let regtest = NetworkConfig {
            active: true,
            api_node: Node{url: "http://127.0.0.1:18443".into(), auth: rpc::Auth::CookieFile(b.into())},
            emulator_nodes: Some(EmulatorConfig{
                enabled: true,
                threshold: 1u8,
                emulators: vec![(ExtendedPubKey::from_str("tpubD6NzVbkrYhZ4Wf398td3H8YhWBsXx9Sxa4W3cQWkNW3N3DHSNB2qtPoUMXrA6JNaPxodQfRpoZNE5tGM9iZ4xfUEFRJEJvfs8W5paUagYCE").unwrap(),
                    "ctv.d31373.org:8367".into())],
            }),
            plugin_map: None,
        };
        ConfigVerifier {
            main: None,
            testnet: None,
            signet: None,
            regtest: Some(regtest),
        }
    }
}
