use super::banner::BANNER;
use anyhow::{anyhow, Error, Result};
use serde::{Deserialize, Serialize};
use std::{
  fs,
  io::{stdin, Write},
  net::UdpSocket,
  path::{Path, PathBuf},
};

const DEFAULT_PORT: u16 = 8888;
const FILE_NAME: &str = "client.yml";
const CONFIG_DIR: &str = ".config";
const APP_CONFIG_DIR: &str = "spotify-tui";
const TOKEN_CACHE_FILE: &str = ".spotify_token_cache.json";

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientConfig {
  pub client_id: String,
  pub client_secret: String,
  pub device_id: Option<String>,
  // FIXME: port should be defined in `user_config` not in here
  pub port: Option<u16>,
}

pub struct ConfigPaths {
  pub config_file_path: PathBuf,
  pub token_cache_path: PathBuf,
}

impl ClientConfig {
  pub fn new() -> ClientConfig {
    ClientConfig {
      client_id: "".to_string(),
      client_secret: "".to_string(),
      device_id: None,
      port: None,
    }
  }

  /// Get the local IP address of this machine.
  /// Falls back to 127.0.0.1 if unable to determine the local IP.
  pub fn get_local_ip() -> String {
    // Use UDP socket to determine local IP by attempting to "connect" to an external address.
    // This doesn't actually send any data, but it allows the OS to select the appropriate
    // network interface and IP address.
    UdpSocket::bind("0.0.0.0:0")
      .and_then(|socket| {
        // Connect to Google's public DNS - this doesn't send any packets
        socket.connect("8.8.8.8:80")?;
        socket.local_addr()
      })
      .map(|addr| addr.ip().to_string())
      .unwrap_or_else(|_| "127.0.0.1".to_string())
  }

  pub fn get_redirect_uri(&self) -> String {
    let ip = ClientConfig::get_local_ip();
    format!("http://{}:{}/callback", ip, self.get_port())
  }

  pub fn get_port(&self) -> u16 {
    self.port.unwrap_or(DEFAULT_PORT)
  }

  pub fn get_or_build_paths(&self) -> Result<ConfigPaths> {
    match dirs::home_dir() {
      Some(home) => {
        let path = Path::new(&home);
        let home_config_dir = path.join(CONFIG_DIR);
        let app_config_dir = home_config_dir.join(APP_CONFIG_DIR);

        if !home_config_dir.exists() {
          fs::create_dir(&home_config_dir)?;
        }

        if !app_config_dir.exists() {
          fs::create_dir(&app_config_dir)?;
        }

        let config_file_path = &app_config_dir.join(FILE_NAME);
        let token_cache_path = &app_config_dir.join(TOKEN_CACHE_FILE);

        let paths = ConfigPaths {
          config_file_path: config_file_path.to_path_buf(),
          token_cache_path: token_cache_path.to_path_buf(),
        };

        Ok(paths)
      }
      None => Err(anyhow!("No $HOME directory found for client config")),
    }
  }

  pub fn set_device_id(&mut self, device_id: String) -> Result<()> {
    let paths = self.get_or_build_paths()?;
    let config_string = fs::read_to_string(&paths.config_file_path)?;
    let mut config_yml: ClientConfig = serde_yaml::from_str(&config_string)?;

    self.device_id = Some(device_id.clone());
    config_yml.device_id = Some(device_id);

    let new_config = serde_yaml::to_string(&config_yml)?;
    let mut config_file = fs::File::create(&paths.config_file_path)?;
    write!(config_file, "{}", new_config)?;
    Ok(())
  }

  pub fn load_config(&mut self) -> Result<()> {
    let paths = self.get_or_build_paths()?;
    if paths.config_file_path.exists() {
      let config_string = fs::read_to_string(&paths.config_file_path)?;
      let config_yml: ClientConfig = serde_yaml::from_str(&config_string)?;

      self.client_id = config_yml.client_id;
      self.client_secret = config_yml.client_secret;
      self.device_id = config_yml.device_id;
      self.port = config_yml.port;

      Ok(())
    } else {
      println!("{}", BANNER);

      println!(
        "Config will be saved to {}",
        paths.config_file_path.display()
      );

      println!("\nHow to get setup:\n");

      let local_ip = ClientConfig::get_local_ip();
      let instructions = [
        "Go to the Spotify dashboard - https://developer.spotify.com/dashboard/applications",
        "Click `Create a Client ID` and create an app",
        "Now click `Edit Settings`",
        &format!(
          "Add `http://{}:{}/callback` to the Redirect URIs",
          local_ip, DEFAULT_PORT
        ),
        "You are now ready to authenticate with Spotify!",
      ];

      let mut number = 1;
      for item in instructions.iter() {
        println!("  {}. {}", number, item);
        number += 1;
      }

      let client_id = ClientConfig::get_client_key_from_input("Client ID")?;
      let client_secret = ClientConfig::get_client_key_from_input("Client Secret")?;

      let mut port = String::new();
      println!("\nEnter port of redirect uri (default {}): ", DEFAULT_PORT);
      stdin().read_line(&mut port)?;
      let port = port.trim().parse::<u16>().unwrap_or(DEFAULT_PORT);

      let config_yml = ClientConfig {
        client_id,
        client_secret,
        device_id: None,
        port: Some(port),
      };

      let content_yml = serde_yaml::to_string(&config_yml)?;

      let mut new_config = fs::File::create(&paths.config_file_path)?;
      write!(new_config, "{}", content_yml)?;

      self.client_id = config_yml.client_id;
      self.client_secret = config_yml.client_secret;
      self.device_id = config_yml.device_id;
      self.port = config_yml.port;

      Ok(())
    }
  }

  fn get_client_key_from_input(type_label: &'static str) -> Result<String> {
    let mut client_key = String::new();
    const MAX_RETRIES: u8 = 5;
    let mut num_retries = 0;
    loop {
      println!("\nEnter your {}: ", type_label);
      stdin().read_line(&mut client_key)?;
      client_key = client_key.trim().to_string();
      match ClientConfig::validate_client_key(&client_key) {
        Ok(_) => return Ok(client_key),
        Err(error_string) => {
          println!("{}", error_string);
          client_key.clear();
          num_retries += 1;
          if num_retries == MAX_RETRIES {
            return Err(Error::from(std::io::Error::new(
              std::io::ErrorKind::Other,
              format!("Maximum retries ({}) exceeded.", MAX_RETRIES),
            )));
          }
        }
      };
    }
  }

  fn validate_client_key(key: &str) -> Result<()> {
    const EXPECTED_LEN: usize = 32;
    if key.len() != EXPECTED_LEN {
      Err(Error::from(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("invalid length: {} (must be {})", key.len(), EXPECTED_LEN,),
      )))
    } else if !key.chars().all(|c| c.is_digit(16)) {
      Err(Error::from(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "invalid character found (must be hex digits)",
      )))
    } else {
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_local_ip_returns_valid_ip() {
    let ip = ClientConfig::get_local_ip();
    // IP should be a valid format (contains dots for IPv4 or colons for IPv6)
    assert!(
      ip.contains('.') || ip.contains(':'),
      "IP address should be valid: {}",
      ip
    );
    // Should not be empty
    assert!(!ip.is_empty());
  }

  #[test]
  fn test_get_redirect_uri_uses_local_ip() {
    let config = ClientConfig::new();
    let redirect_uri = config.get_redirect_uri();
    // Should contain "http://" prefix
    assert!(redirect_uri.starts_with("http://"));
    // Should contain "/callback" suffix
    assert!(redirect_uri.contains("/callback"));
    // Should contain the default port
    assert!(redirect_uri.contains(":8888/"));
    // Should NOT contain "localhost" since we're using IP
    let local_ip = ClientConfig::get_local_ip();
    assert!(
      redirect_uri.contains(&local_ip),
      "Redirect URI should contain local IP: {}",
      redirect_uri
    );
  }

  #[test]
  fn test_get_redirect_uri_with_custom_port() {
    let config = ClientConfig {
      client_id: "".to_string(),
      client_secret: "".to_string(),
      device_id: None,
      port: Some(9999),
    };
    let redirect_uri = config.get_redirect_uri();
    assert!(
      redirect_uri.contains(":9999/"),
      "Redirect URI should contain custom port: {}",
      redirect_uri
    );
  }

  #[test]
  fn test_get_port_default() {
    let config = ClientConfig::new();
    assert_eq!(config.get_port(), 8888);
  }

  #[test]
  fn test_get_port_custom() {
    let config = ClientConfig {
      client_id: "".to_string(),
      client_secret: "".to_string(),
      device_id: None,
      port: Some(3000),
    };
    assert_eq!(config.get_port(), 3000);
  }
}
