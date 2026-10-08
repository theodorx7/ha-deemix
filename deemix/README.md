<h2 align="left">Home Assistant App: Deemix with telegram bot</h2>

<div align="right">
  <a href="https://github.com/theodorx7/ha-deemix#donate"><img src="https://img.shields.io/static/v1?label=DONATE&message=USDT%20&labelColor=555&color=26A17B&style=for-the-badge" alt="DONATE USDT"></a> &thinsp; <a href="https://donate.stream/donate_6a8404d5ea133"><img src="https://img.shields.io/badge/DONAT.stream-fc0?style=for-the-badge&logo=heart&logoColor=white" alt="DONAT.stream"></a>
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


<br/>
  
## SEE DOCUMENTATION TAB FOR MORE DETAILS

<br/>

### ❤️ Support the project
[![DONAT.stream](https://img.shields.io/badge/DONAT.stream-fc0?style=for-the-badge&logo=heart&logoColor=white)](https://donate.stream/donate_6a8404d5ea133)  

![USDT](https://img.shields.io/badge/USDT-26A17B?style=for-the-badge&logo=tether&logoColor=white)  
TRC-20  
<kbd>TQrwpY2LWF96YBbBSZZawRqQ6j9K4PzPQo</kbd>    

BEP-20  
<kbd>0x2a1581bcbd2dc64b9d0f494c636d1d5dacb898e6</kbd>    

POLYGON  
<kbd>0x8051a1cf7a3b41221d723f7eae77d59d14fb275b</kbd>    

TON  
<kbd>EQBetln-nWakoK3LaTOn8l8oqnhNZgbVMHq_neSPPA6tS6nS</kbd>    
