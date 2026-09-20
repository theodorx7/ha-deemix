//! Home Assistant Supervisor API helpers.
//!
//! Used to sync bot runtime changes (bitrate, ARL) back into the add-on
//! options and to restart the add-on from inside the bot.

use std::env;

/// Generic Supervisor API request
async fn supervisor_request(
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let token = env::var("SUPERVISOR_TOKEN")
        .map_err(|_| "SUPERVISOR_TOKEN not set".to_string())?;
    
    let client = reqwest::Client::new();
    let url = format!("http://supervisor{}", path);
    
    let http_method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|e| e.to_string())?;
    
    let mut request = client.request(http_method, &url)
        .header("Authorization", format!("Bearer {}", token));
    
    if let Some(body) = body {
        request = request.json(&body);
    }
    
    let response = request.send().await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Supervisor API error: {}", status));
    }
    
    response.json().await
        .map_err(|e| e.to_string())
}

/// Update an add-on option in HA via the Supervisor API
pub(crate) async fn update_ha_option(key: &str, value: serde_json::Value) -> Result<(), String> {
    // 1. Get current options
    let info = supervisor_request("GET", "/addons/self/info", None).await?;
    let mut options = info["data"]["options"].as_object()
        .ok_or_else(|| "Failed to get addon options".to_string())?
        .clone();
    
    // 2. Update the key
    options.insert(key.to_string(), value);
    
    // 3. Send full options
    let payload = serde_json::json!({ "options": options });
    supervisor_request("POST", "/addons/self/options", Some(payload)).await?;
    
    log::info!("[ha-sync] Updated option '{}' in HA", key);
    Ok(())
}

/// Restart the add-on via the Supervisor API
pub(crate) async fn restart_addon() -> Result<(), String> {
    let info = supervisor_request("GET", "/addons/self/info", None).await?;
    let slug = info["data"]["slug"].as_str()
        .ok_or_else(|| "Failed to get addon slug".to_string())?;
    
    supervisor_request("POST", &format!("/addons/{}/restart", slug), None).await?;
    
    log::info!("[ha-sync] Add-on restart initiated");
    Ok(())
}
