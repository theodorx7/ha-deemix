<h2 align="left">Home Assistant App: Deemix with telegram bot</h2>

<div align="right">
  <a href="https://github.com/theodorx7/ha-deemix#donate"><img src="https://img.shields.io/static/v1?label=DONATE&message=USDT%20&labelColor=555&color=26A17B&style=for-the-badge" alt="DONATE USDT"></a> &thinsp; <a href="https://donate.stream/donate_6a8404d5ea133"><img src="https://img.shields.io/badge/DONAT.stream-fc0?style=for-the-badge&logo=heart&logoColor=white" alt="DONAT.stream"></a>
</div>

[English](https://github.com/theodorx7/ha-deemix/blob/main/README.md) | [Russian](https://github.com/theodorx7/ha-deemix/blob/main/README_RU.md)



## Opening the Web Interface
- Within Home Assistant: on the add-on page, click the "Open Web UI" button.  
  For quick access, enable the `"Show in sidebar"` option.

- Via a direct IP address on the local network, without authenticating or logging into Home Assistant:  
  http://`<your Home Assistant IP address>`:6595 (for example: `http://192.168.1.30:6595`).  
  Only the HTTP protocol is supported. The default port is 6595 — this can be changed in the add-on settings.



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

4. Once the project is created, the ACRCloud console will display your "Host»", "Access Key", "Access Secret".  
   Enter these into the corresponding fields in the add-on settings:  
`ACRCloud Host`  
`ACRCloud Access Key`  
`ACRCloud Secret Key`  

ℹ️ ACRCloud can be used for free, but the number of recognitions (API requests) per month is limited.  



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
