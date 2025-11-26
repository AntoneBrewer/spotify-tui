use rspotify::{prelude::*, AuthCodeSpotify};
use std::{
  io::prelude::*,
  net::{TcpListener, TcpStream},
};

/// Redirect URI web server for OAuth callback
/// Returns the authorization code from the callback URL
pub fn redirect_uri_web_server(spotify: &AuthCodeSpotify, port: u16) -> Result<String, ()> {
  let listener = TcpListener::bind(format!("127.0.0.1:{}", port));

  match listener {
    Ok(listener) => {
      // Get the authorization URL and print it for the user
      match spotify.get_authorize_url(false) {
        Ok(auth_url) => {
          println!("\nPlease open this URL in your browser to authorize:");
          println!("{}\n", auth_url);
          
          // Try to open the URL in the default browser
          if let Err(_) = webbrowser::open(&auth_url) {
            // If automatic opening fails, the user can manually copy the URL
          }
        }
        Err(e) => {
          println!("Failed to generate auth URL: {:?}", e);
          return Err(());
        }
      }

      println!("Waiting for callback on port {}...", port);

      for stream in listener.incoming() {
        match stream {
          Ok(stream) => {
            if let Some(code) = handle_connection(stream) {
              return Ok(code);
            }
          }
          Err(e) => {
            println!("Error: {}", e);
          }
        };
      }
    }
    Err(e) => {
      println!("Failed to bind to port {}: {}", port, e);
    }
  }

  Err(())
}

fn handle_connection(mut stream: TcpStream) -> Option<String> {
  // The request will be quite large (> 512) so just assign plenty just in case
  let mut buffer = [0; 2000];
  let _ = stream.read(&mut buffer).unwrap();

  // convert buffer into string and 'parse' the URL
  match String::from_utf8(buffer.to_vec()) {
    Ok(request) => {
      let split: Vec<&str> = request.split_whitespace().collect();

      if split.len() > 1 {
        let path = split[1];
        
        // Extract the code from the callback URL
        // Format: /callback?code=xxx
        if let Some(code) = extract_code_from_path(path) {
          respond_with_success(stream);
          return Some(code);
        }
        
        respond_with_error("No authorization code found".to_string(), stream);
      } else {
        respond_with_error("Malformed request".to_string(), stream);
      }
    }
    Err(e) => {
      respond_with_error(format!("Invalid UTF-8 sequence: {}", e), stream);
    }
  };

  None
}

fn extract_code_from_path(path: &str) -> Option<String> {
  // Path format: /callback?code=xxx&state=yyy
  if let Some(query_start) = path.find('?') {
    let query = &path[query_start + 1..];
    for param in query.split('&') {
      if let Some(eq_pos) = param.find('=') {
        let key = &param[..eq_pos];
        let value = &param[eq_pos + 1..];
        if key == "code" {
          // URL decode the value and trim any null bytes
          let code = value.trim_matches(char::from(0)).to_string();
          return Some(code);
        }
      }
    }
  }
  None
}

fn respond_with_success(mut stream: TcpStream) {
  let contents = include_str!("redirect_uri.html");

  let response = format!("HTTP/1.1 200 OK\r\n\r\n{}", contents);

  stream.write_all(response.as_bytes()).unwrap();
  stream.flush().unwrap();
}

fn respond_with_error(error_message: String, mut stream: TcpStream) {
  println!("Error: {}", error_message);
  let response = format!(
    "HTTP/1.1 400 Bad Request\r\n\r\n400 - Bad Request - {}",
    error_message
  );

  stream.write_all(response.as_bytes()).unwrap();
  stream.flush().unwrap();
}
