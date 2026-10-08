<div align="center">
  <a href="https://github.com/theodorx7/ha-deemix">
    <img src="deemix/logo.png" alt="Logo" width="180" style="vertical-align: middle; margin-right: 15px;">
  </a>
  <h1 style="display: inline-block; vertical-align: middle; margin: 0;">
    Home Assistant App: Deemix with telegram bot
  </h1>
</div>

<div align="left">
 <img src="https://img.shields.io/badge/aarch64-yes-green.svg" alt="Supports aarch64 Architecture"> &thinsp; <img src="https://img.shields.io/badge/amd64-yes-green.svg" alt="Supports amd64 Architecture">
</div>

<div align="right">
  <a href="#donate"><img src="https://img.shields.io/static/v1?label=DONATE&message=USDT%20&labelColor=555&color=26A17B&style=for-the-badge" alt="DONATE USDT"></a> &thinsp; <a href="https://donate.stream/donate_6a8404d5ea133"><img src="https://img.shields.io/badge/DONAT.stream-fc0?style=for-the-badge&logo=heart&logoColor=white" alt="DONAT.stream"></a>
</div>

[English](https://github.com/theodorx7/ha-deemix/blob/main/README.md) | [Russian](https://github.com/theodorx7/ha-deemix/blob/main/README_RU.md)

This project provides the full functionality of the original (unmodified) [Deemix](https://github.com/bambanah/deemix) as a Home Assistant add-on and extends its capabilities using a Telegram bot.



## Features
- Download music in FLAC (lossless) and MP3 (320 kbps / 128 kbps) – requires a Deezer account (downloading in FLAC and 320 kbps MP3 requires a paid subscription)  

- Batch downloading:

  - one or more selected tracks from an album
  - an entire album or playlist
  - an artist's entire discography  

- Search and download music via the Deemix web interface or Telegram bot (however, for your convenience, searching directly on deezer.com or the Deezer mobile app is recommended)


### Telegram Bot (optional) — requires a [token from @BotFather](https://core.telegram.org/bots/features#creating-a-new-bot)
- Public or private access to the bot via a Telegram ID whitelist  

- Download via link — simply send it to the chat:

  - Deezer: track / album / playlist / artist
  - Spotify: track / album / playlist
  - Apple Music: track / album  

- Music recognition via ACRCloud (a Shazam alternative) – requires an [API key](#acrcloud)

- Search for tracks or albums by sending a text message in the chat  

- Download queue:

  - status check: downloading / pending / completed / failed
  - download error notifications (sent only to the user who requested the download)
  - clear successfully completed downloads  

- Toggle download audio quality: FLAC → MP3 320 → MP3 128 (format/bitrate settings for the Telegram bot and the Deemix web panel are configured independently)  

- Update [Deezer ARL](https://www.dumpmedia.com/deezplus/deezer-arl.html) on the Deemix server



## Installation
### Click the Button
[![Add repository to Home Assistant](https://my.home-assistant.io/badges/supervisor_add_addon_repository.svg)](
https://my.home-assistant.io/redirect/supervisor_add_addon_repository/?repository_url=https://github.com/theodorx7/ha-deemix/
)
### Or Follow the Manual Steps
1. Navigate in your Home Assistant frontend to <kbd>Settings</kbd> → <kbd>Apps</kbd> → <kbd>Install App</kbd> (bottom right).
2. Click the 3-dots menu at upper right <kbd>⋮</kbd> → <kbd>Repositories</kbd> and add this repository's URL: [https://github.com/theodorx7/ha-deemix/](https://github.com/theodorx7/ha-deemix)
3. Refresh the page and find the "Deemix with telegram bot" app.



## Opening the Web Interface
- Within Home Assistant: on the add-on page, click the "Open Web UI" button. For quick access, enable the "Show in sidebar" option.
- Via a direct IP address on the local network, without authenticating or logging into Home Assistant:
  http://`<your Home Assistant IP address>`:6595 (for example: `http://192.168.1.30:6595`). Only the HTTP protocol is supported. The default port is 6595 — this can be changed in the add-on settings.



## Configuration
- The download folder, specified in the add-on settings via an environment variable (env_vars), is applied only during the first launch. After that, the path can only be changed through the Deemix web interface.  

- All available parameters for the original Deemix can be set using environment variables (env_vars) within the add-on settings. A complete list of parameters can be found in the [bambanah/deemix project documentation](https://github.com/bambanah/deemix#parameters).  

- By default, the application runs as user 1000:1000. If necessary, you can set your own `PUID / PGID.` values via an environment variable.  

- By default, the bot downloads in FLAC format, regardless of the Deemix web interface settings. You can change the format/bitrate in the bot settings; however, your selection is not saved after a restart. To override the default download quality, use the `BOT_BITRATE` environment variable with one of the following values: "1" = MP3 128, "3" = MP3 320, "9" = FLAC.

```yaml
env_vars:
  - name: BOT_BITRATE
    value: "3"
```

- The configuration files for Deemix and the Telegram bot are stored in "addon_configs/`<your application slug>`_deemix/"



<a id="acrcloud"></a>
## Setting Up Track Recognition — ACRCloud
Sign up for an ACRCloud account -> https://console.acrcloud.com
Open the ACRCloud Console -> "Audio & Video Recognition" section:

1. In the top-right menu, select one of the 3 regions: Europe, US West, or Asia Pacific

2. In the left sidebar, click "Projects" -> Open the "Audio & Video Recognition" section -> Click "Create Project"

3. Fill in the project creation form:  
	Project Name -> Telegram Bot  
	Audio Source -> Recorded Audio (Audio captured via microphone or noisy audio files)  
	Audio Engine -> Audio Fingerprinting  
	Buckets -> ACRCloud Music  
	The 3rd Party ID Integration -> `spotify` + `deezer` + `isrc`   

⚠️ IMPORTANT: In the «3rd Party ID Integration» option, make sure to check all 3 boxes: `deezer`, `spotify` and `isrc`.  

4. Once the project is created, the ACRCloud console will display your "Host»", "Access Key", "Access Secret". Enter these into the corresponding fields in the add-on settings:  
`ACRCloud Host`  
`ACRCloud Access Key`  
`ACRCloud Secret Key`  

ℹ️ ACRCloud can be used for free, but the number of recognitions (API requests) per month is limited.  



<br/>

<a id="donate"></a>
## ❤️ Support the project
[![DONAT.stream](https://img.shields.io/badge/DONAT.stream-fc0?style=for-the-badge&logo=heart&logoColor=white)](https://donate.stream/donate_6a8404d5ea133)  

![USDT](https://img.shields.io/badge/USDT-26A17B?style=for-the-badge&logo=tether&logoColor=white)  
TRC-20: <kbd>TQrwpY2LWF96YBbBSZZawRqQ6j9K4PzPQo</kbd>   
BEP-20: <kbd>0x2a1581bcbd2dc64b9d0f494c636d1d5dacb898e6</kbd>  
POLYGON: <kbd>0x8051a1cf7a3b41221d723f7eae77d59d14fb275b</kbd>  
TON: <kbd>EQBetln-nWakoK3LaTOn8l8oqnhNZgbVMHq_neSPPA6tS6nS</kbd>  

<br/>
<a href="https://github.com/theodorx7/ha-deemix/">
  <img align="right" alt="Hits" src="https://hits.sh/github.com/theodorx7/ha-deemix.svg?style=for-the-badge&color=555555">
</a>
