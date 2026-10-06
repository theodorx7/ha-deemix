![Supports aarch64 Architecture][aarch64-shield]
![Supports amd64 Architecture][amd64-shield]

[aarch64-shield]: https://img.shields.io/badge/aarch64-yes-green.svg
[amd64-shield]: https://img.shields.io/badge/amd64-yes-green.svg

# Home Assistant App: Deemix with telegram bot

> ⚠️ **WIP: IN DEVELOPMENT**
> Subscribe to updates (**Watch -> Releases**) and wait for the first release.

-------------------------
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
